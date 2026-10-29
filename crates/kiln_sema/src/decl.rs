//! Declarations ([dcl], [basic.def], spec/sema.md §4): decl-specifiers and
//! declarators to types, variables, functions, fields, typedefs, tags,
//! enumerators, namespaces, linkage specifications, `static_assert` and
//! attributes.

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::{
    ConstexprKind, DeclFlags, DeclKind, DefinitionBits, EnumData, FieldData, FnSpec, FnStorage,
    FriendKind, LinkageLang, TagKind, TlsKind, VarSpec, VarStorage,
};
use kiln_ast::expr::{CastKind, ExprKind};
use kiln_ast::ids::RawId;
use kiln_ast::lookup::LookupFlags;
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::types::{
    AutoKeyword, BuiltinType, ExcSpec, FuncInfo, Quals, RefQual, TagKeyword, TypeView,
};
use kiln_ast::{AttrId, DeclHead, DeclId, ExprId, QualType, StmtId};
use kiln_diag::{DiagId, FixIt};
use kiln_source::symbol::Symbol;
use kiln_source::{Loc, Span};

use crate::class::ClassBuild;
use crate::constfold;
use crate::conv::{self, ConvCtx};
use crate::declspec::{
    Chunk, DeclSpec, Declarator, DeclaratorContext, DefaultArg, ParamInfo, Sign, SpecialName,
    StorageSpec, TypeSpecKind, Width,
};
use crate::init::{InitValue, Initializer};
use crate::lookup::{LookupKind, is_tag_kind, is_type_kind};
use crate::scope::{Scope, ScopeKind};
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
    pub fn type_from_declspec_at(&mut self, ds: &DeclSpec, loc: Loc) -> QualType {
        let unsigned = ds.sign == Sign::Unsigned;
        let base = match ds.base {
            TypeSpecKind::None if !ds.has_type_specifier() => {
                if !ds.invalid {
                    self.error(DiagId::parse_missing_type_specifier, loc);
                }
                QualType::INT
            }
            TypeSpecKind::None | TypeSpecKind::Int => match (ds.width, unsigned) {
                (Width::Short, false) => QualType::SHORT,
                (Width::Short, true) => QualType::USHORT,
                (Width::Long, false) => QualType::LONG,
                (Width::Long, true) => QualType::ULONG,
                (Width::LongLong, false) => QualType::LONGLONG,
                (Width::LongLong, true) => QualType::ULONGLONG,
                (Width::None, false) => QualType::INT,
                (Width::None, true) => QualType::UINT,
            },
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
        };
        let cv = ds.cv;
        if cv.is_empty() { base } else { self.ast.types.qualify(base, cv) }
    }

    /// True if `q` is the error type.
    pub fn is_error_type(&self, q: QualType) -> bool {
        self.ast.types.builtin(q) == Some(BuiltinType::Error)
    }

    /// True if `q` is an undeduced `auto`.
    pub fn is_undeduced_auto(&self, q: QualType) -> bool {
        matches!(self.ast.types.view(self.ast.types.canonical(q).ty()), TypeView::Auto { .. })
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
            if self.ast.types.is_array(q) && self.ast.types.array_size(q).is_none() {
                return false;
            }
            return self.is_complete(elem);
        }
        target::is_complete_object(&self.ast, q)
    }

    fn declarator_name(&self, d: &Declarator) -> String {
        if let Some((sp, _)) = d.special {
            return match sp {
                SpecialName::Constructor(n) => sym_str(self, n),
                SpecialName::Destructor(n) => format!("~{}", sym_str(self, n)),
                SpecialName::Operator(op) => format!("operator{}", op.spelling()),
                SpecialName::Conversion(t) => format!("operator {}", self.ty_arg(t)),
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
        let flags = if pack { kiln_ast::decl::ParamData::PACK } else { 0 };
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
        if !member || self.kind_of(self.ctx) != DeclKind::CXXRecord {
            return false;
        }
        let class = self.ctx;
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

    /// Normalizes a parameter list ([dcl.fct]/2): a lone unnamed `void`
    /// parameter means "no parameters"; any other `void` parameter is an
    /// error.
    pub fn act_on_param_list(&mut self, params: Vec<ParamInfo>) -> Vec<ParamInfo> {
        let is_void = |s: &Self, p: &ParamInfo| s.ast.types.is_void(p.ty);
        if params.len() == 1 && params[0].name.is_none() && is_void(self, &params[0]) {
            if !self.ast.types.effective_quals(params[0].ty).is_empty() {
                self.error(DiagId::sema_param_with_void_type, params[0].loc);
            }
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
        let mut ty = base;
        let name = self.declarator_name(d);
        let mut prev_ref = false;
        for chunk in d.chunks.iter().rev() {
            if self.is_error_type(ty) {
                return ty;
            }
            if let Some(which) = self.decltype_auto_compound(ty, chunk) {
                let loc = Self::chunk_loc(chunk);
                let msg = self.diag(DiagId::sema_decltype_auto_compound, loc).arg(which);
                self.emit(msg);
                return QualType::ERROR;
            }
            let t = &self.ast.types;
            match chunk {
                Chunk::Pointer { quals, loc } => {
                    if t.is_reference(ty) {
                        let msg = self
                            .diag(DiagId::sema_pointer_to_reference, *loc)
                            .arg(&name)
                            .arg(self.ty_arg(ty));
                        self.emit(msg);
                        return QualType::ERROR;
                    }
                    let p = self.ast.types.pointer_to(ty);
                    ty = if quals.is_empty() { p } else { self.ast.types.qualify(p, *quals) };
                    prev_ref = false;
                }
                Chunk::MemberPointer { qual, quals, loc } => {
                    prev_ref = false;
                    let Some(qual) = qual else { return QualType::ERROR };
                    let Some(class) = self.member_pointer_class(qual) else {
                        let msg = self.diag(DiagId::sema_mptr_in_nonclass, *loc).arg(&name);
                        self.emit(msg);
                        return QualType::ERROR;
                    };
                    let t = &self.ast.types;
                    if t.is_reference(ty) {
                        let msg = self
                            .diag(DiagId::sema_mptr_to_reference, *loc)
                            .arg(&name)
                            .arg(self.ty_arg(ty));
                        self.emit(msg);
                        return QualType::ERROR;
                    }
                    if t.is_void(ty) {
                        let msg = self.diag(DiagId::sema_mptr_to_void, *loc).arg(&name);
                        self.emit(msg);
                        return QualType::ERROR;
                    }
                    let p = self.ast.types.member_pointer(ty, class.ty());
                    ty = if quals.is_empty() { p } else { self.ast.types.qualify(p, *quals) };
                }
                Chunk::LRef { loc } | Chunk::RRef { loc } => {
                    if t.is_reference(ty) && prev_ref {
                        let msg = self.diag(DiagId::sema_reference_to_reference, *loc).arg(&name);
                        self.emit(msg);
                        return QualType::ERROR;
                    }
                    if t.is_void(ty) {
                        self.error(DiagId::sema_reference_to_void, *loc);
                        return QualType::ERROR;
                    }
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
                    if t.is_void(ty) {
                        let msg = self.diag(DiagId::sema_array_of_void, *loc).arg(&name);
                        self.emit(msg);
                        return QualType::ERROR;
                    }
                    if t.is_function(ty) {
                        let msg = self
                            .diag(DiagId::sema_array_of_functions, *loc)
                            .arg(&name)
                            .arg(self.ty_arg(ty));
                        self.emit(msg);
                        return QualType::ERROR;
                    }
                    if t.is_reference(ty) {
                        let msg = self
                            .diag(DiagId::sema_array_of_references, *loc)
                            .arg(&name)
                            .arg(self.ty_arg(ty));
                        self.emit(msg);
                        return QualType::ERROR;
                    }
                    if !self.require_complete(ty, spec_loc) {
                        let msg = self
                            .diag(DiagId::sema_array_incomplete_type, *loc)
                            .arg(self.ty_arg(ty));
                        self.emit(self.incomplete_note(msg, ty));
                        return QualType::ERROR;
                    }
                    if self.require_non_abstract_element(*loc, ty) {
                        return QualType::ERROR;
                    }
                    ty = match size {
                        None => self.ast.types.incomplete_array(ty),
                        Some(e) if self.expr_is_value_dependent(*e) => {
                            // [temp.dep.constexpr]: checked at instantiation.
                            let p = self.dependent_profile(*e);
                            self.ast.types.dependent_sized_array(ty, *e, p)
                        }
                        Some(e) => match self.array_bound(*e, d) {
                            Some(n) => self.ast.types.constant_array(ty, n, None),
                            None => return QualType::ERROR,
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
                            return QualType::ERROR;
                        }
                        ty = ret;
                        info = info.with_trailing_return(true);
                    }
                    let t = &self.ast.types;
                    if t.is_array(ty) || t.is_function(ty) {
                        let which = if t.is_array(ty) { "0" } else { "1" };
                        let msg = self
                            .diag(DiagId::sema_func_returning_array_function, *loc)
                            .arg(which)
                            .arg(self.ty_arg(ty));
                        self.emit(msg);
                        return QualType::ERROR;
                    }
                    let ptys: Vec<QualType> = params.iter().map(|p| p.ty).collect();
                    ty = self.ast.types.function(ty, &ptys, info, *exc);
                }
            }
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
    /// expression of type `std::size_t`.
    fn array_bound(&mut self, e: ExprId, d: &Declarator) -> Option<u64> {
        let ety = self.ast.exprs.get(e).ty();
        let loc = self.begin_loc(e);
        if self.is_error_type(ety) {
            return None;
        }
        let t = &self.ast.types;
        let integral =
            t.is_integral(ety) || t.as_enum(ety).is_some_and(|en| !self.is_scoped_enum(en));
        if !integral {
            let msg = self.diag(DiagId::sema_array_size_non_int, loc).arg(self.ty_arg(ety));
            self.emit(msg);
            return None;
        }
        let c = conv::decay(self, e);
        let c = self.act_on_full_expr(c);
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
                    let msg = self.diag(DiagId::sema_array_too_large, loc).arg(v.to_string());
                    self.emit(msg);
                    return None;
                }
                u64::try_from(v).ok()
            }
            _ => {
                self.error(DiagId::sema_vla_unsupported, loc);
                None
            }
        }
    }

    pub(crate) fn is_scoped_enum(&self, en: DeclId) -> bool {
        let def = self.ast.definition(en).unwrap_or(en);
        self.ast.decls.enumeration(def).is_some_and(EnumData::is_scoped)
    }

    /// A parameter-declaration ([dcl.fct]/3): its type, adjusted
    /// ([dcl.fct]/5: arrays and functions become pointers).
    pub fn act_on_param_declarator(&mut self, ds: &DeclSpec, d: &Declarator) -> ParamInfo {
        if let Some(l) = d.explicit_object {
            // P0847 (spec/sema-class.md §2.5, Q4).
            self.unsupported("explicit object parameters", l);
        }
        let base = self.type_from_declspec(ds);
        let mut ty = self.type_for_declarator_at(base, ds.type_loc.unwrap_or(ds.loc), d);
        if let Some((s, loc)) = ds.storage
            && s != StorageSpec::Register
        {
            self.error(DiagId::sema_invalid_storage_class_in_func_decl, loc);
        }
        if self.ast.types.is_array(ty) {
            let elem = self.ast.types.element(ty).unwrap_or(QualType::ERROR);
            ty = self.ast.types.pointer_to(elem);
        } else if self.ast.types.is_function(ty) {
            ty = self.ast.types.pointer_to(ty);
        }
        if let Some(l) = d.ellipsis {
            // [dcl.fct]/16: a function parameter pack `T... x`.
            ty = self.act_pack_expansion_type(ty, l);
        }
        let mut attrs = ds.attrs.clone();
        attrs.extend(d.attrs.iter().copied());
        ParamInfo {
            name: d.name,
            ty,
            loc: d.name_loc(),
            attrs,
            invalid: ds.invalid || d.invalid,
            default_arg: DefaultArg::None,
            range: (ds.loc, d.name_loc()),
            decl: None,
        }
    }

    /// The default argument `= init` of parameter `p` ([dcl.fct.default]),
    /// parsed in place: copy-initialization of the parameter's type
    /// ([dcl.fct.default]/5), stored as [`DefaultArg::Parsed`]. (A default
    /// argument replayed in a complete-class context goes through
    /// [`Sema::act_on_param_default_arg`] instead.)
    pub fn act_on_param_default_init(&mut self, p: &mut ParamInfo, init: Initializer) {
        let roots: Vec<ExprId> = match &init {
            Initializer::Expr(e) => vec![*e],
            Initializer::Paren(v, _, _) => v.clone(),
            // (A braced default argument is not searched: a gap.)
            Initializer::List(_) => Vec::new(),
        };
        if !roots.into_iter().all(|e| self.check_default_arg_refs(e)) {
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
                    self.init_list_entity(p.ty, &l, p.loc)
                }
            }
            Initializer::Expr(e) => {
                if self.is_err_expr(e) {
                    e
                } else {
                    match conv::implicit_convert(self, e, p.ty, ConvCtx::Argument) {
                        Ok(x) => x,
                        Err(err) => {
                            let d = err
                                .args
                                .into_iter()
                                .fold(self.diag(err.id, p.loc), kiln_diag::Diag::arg);
                            let d = self.with_param_note(d, p);
                            self.emit(d);
                            let loc = self.ast.exprs.get(e).loc();
                            self.recover(p.ty, &[e], loc)
                        }
                    }
                }
            }
            Initializer::Paren(v, l, _) => self.recover(p.ty, &v, l),
        };
        let e = self.act_on_full_expr(e);
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
        let note = match p.name {
            None => self.diag(DiagId::note_parameter_here, p.loc),
            Some((n, _)) => self
                .diag(DiagId::note_parameter_named_here, p.loc)
                .arg(self.ast.symbols().as_str(n).to_string()),
        };
        d.with_note(note)
    }

    /// True if `q` is an undeduced plain `auto` (not `decltype(auto)`).
    fn is_plain_auto(&self, q: QualType) -> bool {
        q.quals().is_empty()
            && matches!(
                self.ast.types.view(q.ty()),
                TypeView::Auto { keyword: AutoKeyword::Auto, deduced: None, .. }
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

    /// Previous declarations of `name` that `d` may redeclare.
    pub(crate) fn find_prev(&self, name: Symbol, qualifier: Option<DeclId>) -> Option<DeclId> {
        let found: Vec<DeclId> = match qualifier {
            // [class.mfct]/3, [class.static.data]/3: an out-of-line
            // definition redeclares a member of that class itself, never
            // one inherited from a base.
            Some(q) if self.kind_of(q) == DeclKind::CXXRecord => {
                let def = self.ast.definition(q).unwrap_or(q);
                self.lookup_in(def, name, LookupKind::Ordinary).decls
            }
            Some(q) => self.lookup_qualified(q, name, LookupKind::Ordinary).decls,
            None => self.lookup_current_scope(name),
        };
        // A template declaration redeclares a template (its latest
        // pattern); any other declaration skips templates (overloading
        // with templates is WP-071).
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

    /// The previous declaration a function or `extern` variable
    /// declaration redeclares: an ordinary previous declaration, else, at
    /// block scope, one in the innermost enclosing namespace
    /// ([basic.link]/6), else a block-scope declaration of the same entity
    /// ([`Sema::local_externs`]).
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

    /// The innermost namespace (or the translation unit) enclosing the
    /// current context.
    fn enclosing_namespace(&self) -> Option<DeclId> {
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
    /// for later namespace-scope redeclarations.
    fn note_local_extern(&mut self, name: Symbol, d: DeclId) {
        if self.in_local_scope()
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
        if let Some(l) = ds.consteval {
            self.unsupported("'consteval'", l);
        }
        // A friend function goes through `act_on_friend`; elsewhere
        // `friend` is not in a class.
        if d.ctx != DeclaratorContext::Member
            && let Some(l) = ds.friend
        {
            self.unsupported("'friend'", l);
        }
        // Member functions take cv/ref-qualifiers (WP-066b); a non-member
        // function type with them is `typedef`-only ([dcl.fct]/6).
        if d.ctx != DeclaratorContext::Member
            && !ds.is_typedef()
            && !d.invalid
            && let Some(Chunk::Function { quals, ref_qual, .. }) = d.chunks.first()
            && (!quals.is_empty() || *ref_qual != RefQual::None)
            && d.qualifier.is_none_or(|q| self.ast.decls.record(q).is_none())
        {
            self.unsupported("function qualifiers on a non-member function", d.name_loc());
        }
        false
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
    pub fn act_on_declarator(
        &mut self,
        ds: &DeclSpec,
        base: QualType,
        d: &Declarator,
    ) -> Option<DeclId> {
        self.finish_invented_head();
        let head = self.take_template_head(d.qualifier);
        let special = !d.dropped && self.handles_special_declarator(ds, d);
        if d.dropped || !special && self.unsupported_declarator(ds, d) {
            self.templ.cur = None;
            return None;
        }
        let ty = self.type_for_declarator_at(base, ds.type_loc.unwrap_or(ds.loc), d);
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
        let invalid = ds.invalid || d.invalid || self.is_error_type(ty);
        if special {
            return self.act_on_special_member_declarator(ds, ty, d, invalid);
        }
        if !invalid {
            self.warn_ambiguous_function(ds, d, ty);
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
        let mut attrs = ds.attrs.clone();
        attrs.extend(d.attrs.iter().copied());
        if d.op_name.is_some()
            && (ds.is_typedef()
                || !self.ast.types.is_function(ty)
                || d.ctx == DeclaratorContext::Member
                || d.qualifier.is_some())
        {
            // An operator-function-id names a function at namespace scope
            // here; member operators are WP-068b.
            self.templ.cur = None;
            self.unsupported("this declaration of an operator function", d.loc);
            return None;
        }
        if ds.is_typedef() || !self.ast.types.is_function(ty) {
            self.virtual_on_non_function(ds, d);
        }
        let decl = if ds.is_typedef() {
            self.act_on_typedef(ds, ty, d, name, false)
        } else if self.ast.types.is_function(ty) {
            self.act_on_function(ds, ty, d, name, false)
        } else if d.ctx == DeclaratorContext::Member
            && !matches!(ds.storage, Some((StorageSpec::Static, _)))
        {
            self.act_on_field(ds, ty, d, invalid)
        } else {
            // [dcl.constexpr]/6: a constexpr object is const.
            let ty = if ds.constexpr.is_some() && !self.ast.types.is_reference(ty) {
                self.ast.types.qualify(ty, Quals::CONST)
            } else {
                ty
            };
            self.act_on_var(ds, ty, d, name)
        };
        if invalid {
            self.ast.decls.set_flags(decl, DeclFlags::INVALID);
        }
        if let Some(r) = d.trailing_requires {
            self.act_on_trailing_requires(decl, r);
        }
        self.set_member_access(decl);
        if let Some(t) = self.finish_template_pattern(decl) {
            self.set_member_access(t);
        }
        self.act_on_asm_label(ds, d, decl, &mut attrs);
        if d.qualifier.is_none() && d.op_name.is_none() {
            self.check_using_shadow_conflict(decl, name);
        }
        if !attrs.is_empty() {
            self.check_decl_attrs(decl, &attrs);
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
        if let Some(l) = ds.consteval {
            self.unsupported("'consteval'", l);
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
        if let Some(t) = self.finish_template_pattern(decl) {
            self.set_member_access(t);
        }
        let mut attrs = ds.attrs.clone();
        attrs.extend(d.attrs.iter().copied());
        if !attrs.is_empty() {
            self.check_decl_attrs(decl, &attrs);
            let _ = self.builder().attach_attrs(decl, &attrs);
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
        if !matches!(kind, DeclKind::Function | DeclKind::Var) {
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
            let what = if kind == DeclKind::Function { "1" } else { "0" };
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

    /// Reports an unsupported construct (`{what} is not supported yet`).
    pub fn unsupported(&self, what: &str, loc: Loc) {
        let d = self.diag(DiagId::parse_unsupported, loc).arg(what);
        self.emit(d);
    }

    /// Whether function types `a` and `b` are the same apart from their
    /// exception specifications.
    pub(crate) fn same_ignoring_exc_spec(&mut self, a: QualType, b: QualType) -> bool {
        let t = &self.ast.types;
        let (Some(pa), Some(pb)) = (t.proto(a).copied(), t.proto(b).copied()) else {
            return false;
        };
        let (ra, rb) = (t.params(&pa).to_vec(), t.params(&pb).to_vec());
        let a = self.ast.types.function(pa.ret(), &ra, pa.info(), ExcSpec::None);
        let b = self.ast.types.function(pb.ret(), &rb, pb.info(), ExcSpec::None);
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
        msg.with_note(self.diag(id, loc))
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
        if self.ast.definition(rec).is_some() {
            return msg;
        }
        if self.inst.undefined_diagnosed.get() == Some(rec) {
            self.inst.undefined_diagnosed.set(None);
            self.inst.suppress_next_error.set(true);
            return msg;
        }
        let loc = self.ast.decls.get(rec).loc();
        msg.with_note(
            self.diag(DiagId::note_forward_declaration, loc)
                .arg(format!("'{}'", self.qualified_decl_name(rec))),
        )
    }

    pub(crate) fn register_local(&mut self, name: Symbol, d: DeclId) {
        if self.in_local_scope() {
            self.push_local(name, d);
        }
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
        let mut invalid = false;
        if let Some(p) = prev {
            let pk = self.ast.decls.get(p).kind();
            if matches!(pk, DeclKind::Typedef | DeclKind::TypeAlias) {
                let pty = self.ast.decls.typedef(p).map_or(QualType::ERROR, |t| t.underlying);
                if self.ast.types.same_type(pty, underlying) {
                    prev_ok = Some(p);
                } else {
                    let msg = self
                        .diag(DiagId::sema_redefinition_different_typedef, d.name_loc())
                        .arg(if alias { "1" } else { "0" })
                        .arg(self.ty_arg(underlying))
                        .arg(self.ty_arg(pty));
                    let msg = self.note_prev(msg, p, true);
                    self.emit(msg);
                    invalid = true;
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
        if invalid {
            self.ast.decls.set_flags(td, DeclFlags::INVALID);
        }
        if let Some(t) = anon_tag {
            let _ = self.ast.decls.set_typedef_for_anon(t, td);
        }
        self.register_local(name, td);
        td
    }

    /// `using name = type-id;` ([dcl.typedef]/2).
    pub fn act_on_alias_decl(
        &mut self,
        name: Symbol,
        name_loc: Loc,
        ty: QualType,
        attrs: &[AttrId],
    ) -> DeclId {
        let mut d = Declarator::new(DeclaratorContext::File, name_loc);
        d.name = Some((name, name_loc));
        let ds = DeclSpec::new(name_loc);
        let _ = self.take_template_head(None);
        let td = self.act_on_typedef(&ds, ty, &d, name, true);
        if let Some(t) = self.finish_template_pattern(td) {
            self.set_member_access(t);
        }
        self.check_using_shadow_conflict(td, name);
        if !attrs.is_empty() {
            let _ = self.builder().attach_attrs(td, attrs);
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
            Some((StorageSpec::Mutable, loc)) if method => {
                self.error(DiagId::sema_mutable_function, loc);
            }
            Some((StorageSpec::Register | StorageSpec::Mutable, loc)) => {
                self.error(DiagId::sema_invalid_storage_class_in_func_decl, loc);
            }
            _ => {}
        }
        let ty = if in_class && spec.storage() == FnStorage::Static {
            self.check_static_method_quals(ty, d)
        } else {
            ty
        };
        if ds.inline.is_some() {
            spec = spec.with_inline(true);
        }
        if ds.constexpr.is_some() {
            spec = spec.with_constexpr_kind(ConstexprKind::Constexpr).with_implicit_inline(true);
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
        }
        let mut prev_ok = None;
        let mut invalid_redecl = false;
        let fkind = if method { DeclKind::CXXMethod } else { DeclKind::Function };
        let prev = if d.op_name.is_some() {
            None
        } else if friend && d.qualifier.is_none() {
            // Redeclaration lookup in the namespace sees hidden friends.
            self.ast
                .lookup_local(ctx, DeclName::ident(name))
                .iter()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .find(|&p| !is_tag_kind(self.kind_of(p)))
        } else if method {
            self.find_prev(name, d.qualifier)
        } else {
            self.find_prev_linkage(name, d.qualifier)
        };
        let matches_member = prev.is_some_and(|p| {
            self.kind_of(p) == DeclKind::CXXMethod
                && self
                    .ast
                    .decls
                    .get(p)
                    .ty()
                    .is_some_and(|pty| self.same_ignoring_exc_spec(pty, ty))
        });
        let mut ty = ty;
        if let Some(op) = d.op_name {
            // A replaceable global allocation or deallocation function
            // redeclares the implicit one ([new.delete]/2).
            if !method
                && !friend
                && matches!(
                    op,
                    OverloadedOperator::New
                        | OverloadedOperator::ArrayNew
                        | OverloadedOperator::Delete
                        | OverloadedOperator::ArrayDelete
                )
            {
                let (t, p) = self.act_on_global_new_delete_decl(op, ty);
                ty = t;
                prev_ok = p;
            } else {
                // todo(WP-068b): other operator functions, class-specific
                // and friend operator functions.
                self.unsupported(
                    "operator functions other than 'operator new' and 'operator delete'",
                    d.name_loc(),
                );
            }
        } else if friend && d.qualifier.is_some() && (prev.is_none() || method && !matches_member) {
            let msg = self
                .diag(DiagId::sema_friend_def_no_match, d.name_loc())
                .arg(sym_str(self, name))
                .arg(self.qualifier_desc(ctx));
            self.emit(msg);
            invalid_redecl = true;
        } else if method && !in_class && !matches_member
            || !method && !friend && d.qualifier.is_some() && prev.is_none()
        {
            // [dcl.meaning.general]/2: a qualified declarator-id names a
            // previously declared member of the nominated class or
            // namespace.
            let scope = if method {
                format!("'{}'", self.qualified_decl_name(ctx))
            } else {
                self.qualifier_desc(ctx)
            };
            let msg = self
                .diag(DiagId::sema_member_def_no_match, d.name_loc())
                .arg(sym_str(self, name))
                .arg(scope);
            self.emit(msg);
            invalid_redecl = true;
        } else if let Some(p) = prev
            && in_class
            && self.kind_of(p) == DeclKind::CXXMethod
        {
            // [class.mem.general]/5: a member function is declared once
            // in its class (overloads are WP-068).
            let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
            let msg = if self.ast.types.same_type(pty, ty) {
                self.diag(DiagId::sema_member_redeclared, d.name_loc())
            } else {
                self.diag(DiagId::sema_overload_unsupported, d.name_loc()).arg(sym_str(self, name))
            };
            let msg = self.note_prev(msg, p, false);
            self.emit(msg);
            invalid_redecl = true;
        } else if let Some(p) = prev {
            let pn = *self.ast.decls.get(p);
            if pn.kind() == fkind {
                let pty = pn.ty().unwrap_or(QualType::ERROR);
                if self.ast.types.same_type(pty, ty) {
                    prev_ok = Some(p);
                    let pspec = self.ast.decls.function(p).map(|f| f.spec).unwrap_or_default();
                    if spec.storage() == FnStorage::Static && pspec.storage() != FnStorage::Static {
                        let msg = self
                            .diag(DiagId::sema_static_non_static, d.name_loc())
                            .arg(sym_str(self, name));
                        let msg = self.note_prev(msg, p, false);
                        self.emit(msg);
                    }
                    if pspec.storage() == FnStorage::Static && spec.storage() == FnStorage::None {
                        spec = spec.with_storage(FnStorage::Static).with_storage_inherited(true);
                    }
                    if pspec.is_inline() {
                        spec = spec.with_implicit_inline(true);
                    }
                } else if self.same_ignoring_exc_spec(pty, ty) {
                    // [except.spec]/5: every declaration of a function with
                    // a non-throwing specification has one, and only those.
                    let msg = match self.written_exc_spec(ty) {
                        ExcSpec::None => {
                            let spelled = self.exc_spec_spelling(self.written_exc_spec(pty));
                            self.diag(DiagId::sema_missing_exception_spec, d.name_loc())
                                .arg(sym_str(self, name))
                                .arg(spelled)
                        }
                        _ => self.diag(DiagId::sema_exception_spec_mismatch, d.name_loc()),
                    };
                    let msg = self.note_prev(msg, p, false);
                    self.emit(msg);
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
                    let msg = if same_params {
                        self.diag(DiagId::sema_ovl_diff_return_type, d.name_loc())
                    } else {
                        self.diag(DiagId::sema_overload_unsupported, d.name_loc())
                            .arg(sym_str(self, name))
                    };
                    // clang's `getNoteDiagForInvalidRedeclaration`.
                    let def = self.ast.decls.function(p).is_some_and(|f| f.body.is_some());
                    let msg = self.note_prev(msg, p, def);
                    self.emit(msg);
                }
            } else {
                let msg = self
                    .diag(DiagId::sema_redefinition_different_kind, d.name_loc())
                    .arg(sym_str(self, name));
                let msg = self.note_prev(msg, p, true);
                self.emit(msg);
            }
        }
        let params: Vec<ParamInfo> =
            d.function_params().map(<[ParamInfo]>::to_vec).unwrap_or_default();
        let (defaults, redefined) = self.merge_default_args(prev_ok, block, &params);
        // clang marks the redeclaration invalid and leaves it out of the
        // redeclaration chain; calls ignore it.
        if redefined {
            prev_ok = None;
            invalid_redecl = true;
        }
        let dname = d.op_name.map_or(DeclName::ident(name), DeclName::operator);
        let mut h = DeclHead::new(ctx, dname, d.name_loc());
        h.lexical = Some(lexical);
        if friend {
            // [namespace.memdef]/3: visible to ordinary lookup only if
            // declared outside the class (before or after).
            let visible = prev_ok.is_some_and(|p| {
                self.ast
                    .lookup_local(ctx, DeclName::ident(name))
                    .iter_with_flags()
                    .any(|(x, fl)| x == p && !fl.contains(LookupFlags::ADL_ONLY))
            });
            let k = if visible || d.qualifier.is_some() {
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
        if invalid_redecl {
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
        }
        if d.op_name.is_none() {
            self.note_local_extern(name, f);
        }
        self.create_params(f, &params, &defaults);
        if d.op_name.is_none() {
            self.register_local(name, f);
        }
        f
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
            let flags = if pack { kiln_ast::decl::ParamData::PACK } else { 0 };
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
            if p.range.0.is_valid() && p.range.0 != p.loc {
                let _ = self.ast.decls.cold.begin_loc.insert(pd, p.range.0);
            }
            if p.invalid {
                self.ast.decls.set_flags(pd, DeclFlags::INVALID);
            }
            if !p.attrs.is_empty() {
                let _ = self.builder().attach_attrs(pd, &p.attrs);
            }
            match p.default_arg {
                // Replayed in the complete-class context through
                // `act_on_param_default_arg` (spec/sema-class.md §2.7).
                DefaultArg::Unparsed(_) => self.ast.decls.mark_default_arg_pending(pd),
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
    /// a namespace-scope function) have distinct sets. Then every
    /// parameter after one with a default argument needs one.
    pub(crate) fn merge_default_args(
        &mut self,
        prev: Option<DeclId>,
        block: bool,
        params: &[ParamInfo],
    ) -> (Vec<Option<ExprId>>, bool) {
        let mut out: Vec<Option<ExprId>> = params
            .iter()
            .map(|p| match p.default_arg {
                DefaultArg::Parsed(e) => Some(e),
                DefaultArg::None | DefaultArg::Unparsed(_) => None,
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
        if let Some(first) = out.iter().position(Option::is_some) {
            for (i, p) in params.iter().enumerate().skip(first + 1) {
                if out[i].is_some() || p.default_arg != DefaultArg::None || p.invalid {
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
        let mut flags = 0;
        if let Some((s, loc)) = ds.storage {
            match s {
                StorageSpec::Mutable => {
                    // [dcl.stc]/9: not on a const object or a reference.
                    if self.ast.types.is_reference(ty) {
                        self.error(DiagId::sema_mutable_reference, loc);
                    } else if self.ast.types.effective_quals(ty).is_const() {
                        self.error(DiagId::sema_mutable_const, loc);
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
        if let Some(loc) = ds.constexpr {
            self.error(DiagId::sema_constexpr_nonstatic_member, loc);
        }
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
        if let Some(n) = name
            && let Some(&prev) = self.lookup_in(ctx, n, LookupKind::Ordinary).decls.first()
        {
            let note = self.diag(DiagId::note_previous_declaration, self.ast.decls.get(prev).loc());
            let msg = self.diag(DiagId::sema_duplicate_member, d.name_loc()).arg(sym_str(self, n));
            self.emit(msg.with_note(note));
        }
        // [dcl.spec.auto.general]/5: a placeholder is not allowed in a
        // non-static data member (clang's `err_auto_not_allowed`).
        let invalid = invalid || {
            let kw = self.placeholder_keyword(ty);
            if let Some(kw) = kw {
                let tag = self.ast.decls.record(ctx).map_or(TagKind::Struct, |r| r.tag);
                let msg = self
                    .diag(DiagId::sema_auto_not_allowed_field, ds.type_loc.unwrap_or(ds.loc))
                    .arg(u8::from(kw == AutoKeyword::DecltypeAuto))
                    .arg(tag as u8);
                self.emit(msg);
            }
            kw.is_some()
        };
        let ty = if invalid && self.placeholder_keyword(ty).is_some() {
            QualType::ERROR
        } else if !invalid
            && !self.ast.types.is_reference(ty)
            && !self.require_complete(ty, d.name_loc())
        {
            let msg = self.diag(DiagId::sema_field_incomplete, d.name_loc()).arg(self.ty_arg(ty));
            self.emit(self.incomplete_note(msg, ty));
            // clang marks the field invalid: initialization never
            // descends into it (a field of the class's own type would
            // recurse forever).
            QualType::ERROR
        } else if !invalid
            && self.require_non_abstract(d.name_loc(), ty, crate::class::AbstractUse::Field)
        {
            QualType::ERROR
        } else {
            ty
        };
        let width = d.bit_width.and_then(|w| self.bit_width(w, ty, nm.as_deref(), d.name_loc()));
        let h = DeclHead::new(ctx, name.map_or(DeclName::EMPTY, DeclName::ident), d.name_loc());
        let f = self.builder().field(h, ty, width, flags);
        if d.init_style.is_some() {
            self.special.nsdmi.insert(f);
        }
        f
    }

    /// Checks a bit-field width ([class.bit]/1): an integral constant
    /// expression, wrapped in a `ConstantExpr`.
    /// Diagnostics point at the field's location `loc`, as clang's do.
    fn bit_width(
        &mut self,
        w: ExprId,
        ty: QualType,
        name: Option<&str>,
        loc: Loc,
    ) -> Option<ExprId> {
        if self.expr_is_value_dependent(w) || self.is_dependent_type(ty) {
            return Some(w);
        }
        let t = &self.ast.types;
        if !(t.is_integral(ty) || t.as_enum(ty).is_some() || self.is_error_type(ty)) {
            let msg = match name {
                Some(n) => {
                    self.diag(DiagId::sema_bitfield_non_integral, loc).arg(n).arg(self.ty_arg(ty))
                }
                None => {
                    self.diag(DiagId::sema_anon_bitfield_non_integral, loc).arg(self.ty_arg(ty))
                }
            };
            self.emit(msg);
            return None;
        }
        let c = conv::decay(self, w);
        let c = self.act_on_full_expr(c);
        let v = match self.eval_int(c, EvalContext::new(EvalKind::Other, self.begin_loc(w))) {
            Ok(v) => v,
            Err(f) => {
                let d = self.diag(DiagId::sema_expr_not_ice, self.begin_loc(w));
                let d = Self::with_failure_notes(d, f);
                self.emit(d);
                return None;
            }
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
            let msg = self.diag(DiagId::sema_bitfield_zero_width, loc).arg(n);
            self.emit(msg);
            return None;
        }
        if let Some((bits, _)) = constfold::int_info(&self.ast, ty) {
            let bits = if self.ast.types.builtin(ty) == Some(BuiltinType::Bool) { 8 } else { bits };
            if v > i128::from(bits)
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

    pub(crate) fn var_spec(&mut self, ds: &DeclSpec, d: &Declarator) -> VarSpec {
        let mut spec = VarSpec::new();
        if let Some(style) = d.init_style {
            spec = spec.with_init_style(style);
        }
        match ds.storage {
            Some((StorageSpec::Static, _)) => spec = spec.with_storage(VarStorage::Static),
            Some((StorageSpec::Extern, _)) => spec = spec.with_storage(VarStorage::Extern),
            Some((StorageSpec::Mutable, loc)) => {
                self.error(DiagId::sema_invalid_storage_class_in_func_decl, loc);
            }
            _ => {}
        }
        if ds.thread_local.is_some() {
            spec = spec.with_tls(TlsKind::Static);
        }
        if ds.constexpr.is_some() {
            spec = spec.with_constexpr(true);
        }
        if ds.constinit.is_some() {
            spec = spec.with_constinit(true);
        }
        if ds.inline.is_some() {
            spec = spec.with_inline(true);
        }
        spec
    }

    pub(crate) fn is_var_definition(&self, v: DeclId) -> bool {
        // [basic.def]/2.4: a static data member declared in its class is
        // a definition only when inline (constexpr implies inline).
        if self.is_in_class_static(v) {
            return self.ast.decls.var(v).is_some_and(|x| x.spec.is_inline());
        }
        self.ast
            .decls
            .var(v)
            .is_some_and(|x| x.spec.storage() != VarStorage::Extern || x.init.is_some())
    }

    fn act_on_var(&mut self, ds: &DeclSpec, ty: QualType, d: &Declarator, name: Symbol) -> DeclId {
        let (ctx, lexical) = self.decl_contexts(d);
        let spec = self.var_spec(ds, d);
        if self.kind_of(ctx) == DeclKind::CXXRecord {
            return self.act_on_static_member(ds, ty, d, name, spec);
        }
        let mut prev_ok = None;
        let local = self.find_prev(name, d.qualifier);
        let prev = match local {
            None if spec.storage() == VarStorage::Extern || !self.in_local_scope() => {
                self.find_prev_linkage(name, d.qualifier)
            }
            p => p,
        };
        if let Some(p) = prev {
            let pn = *self.ast.decls.get(p);
            let block = local.is_some() && self.in_local_scope();
            if pn.kind() == DeclKind::Var && !block {
                let pty = pn.ty().unwrap_or(QualType::ERROR);
                let compatible = self.ast.types.same_type(pty, ty) || {
                    let t = &self.ast.types;
                    t.is_array(pty)
                        && t.is_array(ty)
                        && (t.array_size(pty).is_none() || t.array_size(ty).is_none())
                        && t.element(pty).zip(t.element(ty)).is_some_and(|(a, b)| t.same_type(a, b))
                };
                if !compatible {
                    // clang's `diagnoseVarDeclTypeMismatch`: "redefinition" if
                    // the new declaration is a definition (judged before its
                    // initializer is seen), and the note says whether the
                    // old one was.
                    let new_def = spec.storage() != VarStorage::Extern;
                    let id = if new_def {
                        DiagId::sema_redefinition_different_type
                    } else {
                        DiagId::sema_redeclaration_different_type
                    };
                    let msg = self
                        .diag(id, d.name_loc())
                        .arg(sym_str(self, name))
                        .arg(self.ty_arg(ty))
                        .arg(self.ty_arg(pty));
                    let msg = self.note_prev(msg, p, self.is_var_definition(p));
                    self.emit(msg);
                } else if spec.storage() != VarStorage::Extern && self.is_var_definition(p) {
                    let msg =
                        self.diag(DiagId::sema_redefinition, d.name_loc()).arg(sym_str(self, name));
                    let msg = self.note_prev(msg, p, true);
                    self.emit(msg);
                } else {
                    prev_ok = Some(p);
                }
            } else if block && matches!(pn.kind(), DeclKind::Var | DeclKind::ParmVar) {
                let msg =
                    self.diag(DiagId::sema_redefinition, d.name_loc()).arg(sym_str(self, name));
                let msg = self.note_prev(msg, p, true);
                self.emit(msg);
            } else {
                let msg = self
                    .diag(DiagId::sema_redefinition_different_kind, d.name_loc())
                    .arg(sym_str(self, name));
                let msg = self.note_prev(msg, p, true);
                self.emit(msg);
            }
        }
        if let Some(p) = prev_ok {
            // An incomplete array redeclared with a bound takes the bound.
            let _ = p;
        }
        let mut h = DeclHead::new(ctx, DeclName::ident(name), d.name_loc());
        h.lexical = Some(lexical);
        self.mark_pattern_next();
        let v = self.builder().var(h, ty, spec, prev_ok);
        self.check_abstract_var(v);
        self.register_local(name, v);
        if spec.storage() == VarStorage::Extern {
            self.note_local_extern(name, v);
        }
        v
    }

    // -- initializers ---------------------------------------------------------

    /// The initializer of variable `v` ([dcl.init]). Its syntax
    /// ([`Declarator::init_style`]) was recorded when `v` was declared.
    pub fn act_on_init(&mut self, v: DeclId, init: Initializer) {
        self.act_on_init_inner(v, init);
        self.initializing = None;
    }

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
        let spec = self.ast.decls.var(v).map(|x| x.spec).unwrap_or_default();
        let loc = vn.loc();
        if spec.storage() == VarStorage::Extern {
            if self.in_local_scope() {
                self.error(DiagId::sema_block_extern_init, loc);
                return;
            }
            self.error(DiagId::sema_extern_init, loc);
        }
        let mut ty = vn.ty().unwrap_or(QualType::ERROR);
        let direct = spec.init_style() == kiln_ast::decl::InitStyle::List;
        if self.contains_undeduced_auto(ty) {
            ty = match self.deduce_var_auto(v, ty, &init, loc, direct) {
                Some(t) => t,
                None => QualType::ERROR,
            };
            self.ast.decls.set_ty(v, ty);
            // `auto x = {...};` deduced `std::initializer_list<U>`; its
            // initialization (`CXXStdInitializerListExpr` over a backing
            // array) lands with WP-068a (spec/sema-class.md §5.7).
            if let Initializer::List(l) = &init
                && !direct
                && !self.is_error_type(ty)
                && !self.ast.types.is_dependent(ty)
            {
                self.unsupported("std::initializer_list", l.lbrace);
                for e in Self::list_exprs(l) {
                    self.act_on_full_expr(e);
                }
                return;
            }
            // `auto x = f();` with a `void` f deduces an incomplete
            // object type (clang `DeduceVariableDeclarationType`, then
            // `err_typecheck_decl_incomplete_type`); `x` gets no initializer.
            if self.ast.types.is_void(ty) {
                let msg = self.diag(DiagId::sema_void_var, loc).arg(self.ty_arg(ty));
                self.emit(msg);
                self.ast.decls.set_flags(v, DeclFlags::INVALID);
                let es = match &init {
                    Initializer::Expr(e) => vec![*e],
                    Initializer::Paren(es, ..) => es.clone(),
                    Initializer::List(l) => Self::list_exprs(l),
                };
                for e in es {
                    self.act_on_full_expr(e);
                }
                return;
            }
        }
        if let Initializer::List(l) = &init
            && (self.ast.types.is_dependent(ty) || self.init_list_type_dependent(l))
        {
            // Initialized at instantiation ([temp.dep.expr]); clang keeps
            // the list as written with type `void` (and `int[]` unbounded).
            let e = self.dependent_init_list(l);
            let e = self.act_on_full_expr(e);
            let _ = self.builder().set_var_init(v, e);
            return;
        }
        let e = self.perform_init(ty, init, loc, direct);
        let e = self.act_on_full_expr(e);
        let _ = self.builder().set_var_init(v, e);
        // An incomplete array takes its bound from the initializer.
        let ety = self.ast.exprs.get(e).ty();
        if self.ast.types.is_array(ty)
            && self.ast.types.array_size(ty).is_none()
            && self.ast.types.array_size(ety).is_some()
        {
            self.ast.decls.set_ty(v, ety);
        }
        if spec.is_constexpr() {
            self.check_constexpr_init(v, e);
        } else if !spec.is_inline() && self.is_in_class_static(v) {
            self.check_in_class_static_init(v, e);
        }
    }

    /// `auto x = e;` ([dcl.type.auto.deduct]): deduces through the
    /// `deduce_auto` seam (`templ/auto.rs`) and reports a failure as
    /// clang's `DeduceVariableDeclarationType` does.
    fn deduce_var_auto(
        &mut self,
        v: DeclId,
        declared: QualType,
        init: &Initializer,
        loc: Loc,
        direct: bool,
    ) -> Option<QualType> {
        use crate::templ::auto::{AutoFail, AutoInit};
        let list_items = |l: &crate::init::InitList| -> Vec<Option<ExprId>> {
            l.items
                .iter()
                .map(|it| match it.value {
                    InitValue::Expr(e) if it.designator.is_empty() => Some(e),
                    _ => None,
                })
                .collect()
        };
        let (items, kind, lbrace) = match init {
            Initializer::Expr(e) => (vec![Some(*e)], AutoInit::Expr, None),
            Initializer::Paren(es, ..) => {
                (es.iter().copied().map(Some).collect(), AutoInit::Paren, None)
            }
            Initializer::List(l) if direct => (list_items(l), AutoInit::DirectList, Some(l.lbrace)),
            Initializer::List(l) => (list_items(l), AutoInit::CopyList, Some(l.lbrace)),
        };
        let fail = match self.deduce_auto(declared, &items, kind) {
            Ok(t) => return Some(t),
            Err(f) => f,
        };
        let lbrace = lbrace.unwrap_or(loc);
        let name = self.decl_name(v);
        let msg = match fail {
            AutoFail::Error => return Some(QualType::ERROR),
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
            AutoFail::Conflict(a, b, _) => self
                .diag(DiagId::sema_auto_inconsistent_deduction, lbrace)
                .arg(self.ty_arg(a))
                .arg(self.ty_arg(b)),
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

    /// A `constexpr` variable's initializer must be a constant expression
    /// ([dcl.constexpr]/10); its value is recorded.
    fn check_constexpr_init(&mut self, v: DeclId, e: ExprId) {
        if self.ast.decls.begin_eval(v).is_err() {
            return;
        }
        // clang reports at the variable (`CheckCompleteVariableDeclaration`).
        let loc = self.ast.decls.get(v).loc();
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
            Err(f) => {
                self.ast.decls.finish_eval(v, None);
                let name = self.decl_name(v);
                let msg = self.diag(DiagId::sema_constexpr_var_non_constant, loc).arg(name);
                let msg = Self::with_failure_notes(msg, f);
                self.emit(msg);
            }
        }
    }

    pub(crate) fn has_errors_in(&self, e: ExprId) -> bool {
        self.ast.exprs.get(e).kind() == ExprKind::RecoveryExpr
            || self.is_error_type(self.ast.exprs.get(e).ty())
    }

    /// A variable declared without an initializer ([dcl.init]/7).
    pub fn act_on_uninit(&mut self, v: DeclId) {
        self.initializing = None;
        let vn = *self.ast.decls.get(v);
        if vn.kind() != DeclKind::Var || vn.has(DeclFlags::INVALID) {
            return;
        }
        // An in-class static data member declaration is not a definition
        // ([class.static.data]/3): its type may be incomplete.
        if self.is_in_class_static(v) && !self.is_var_definition(v) {
            return;
        }
        // [depr.static.constexpr]: a redundant out-of-class redeclaration
        // of an inline static data member initialized in its class.
        if !self.is_in_class_static(v)
            && self
                .ast
                .redecls_of(v)
                .any(|x| x != v && self.is_in_class_static(x) && self.is_var_definition(x))
        {
            return;
        }
        let spec = self.ast.decls.var(v).map(|x| x.spec).unwrap_or_default();
        let ty = vn.ty().unwrap_or(QualType::ERROR);
        let loc = vn.loc();
        let name = self.decl_name(v);
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
            let msg =
                self.diag(DiagId::sema_auto_var_requires_init, loc).arg(name).arg(self.ty_arg(ty));
            self.emit(msg);
            return;
        }
        if self.ast.types.is_reference(ty) {
            if spec.storage() != VarStorage::Extern {
                let msg = self.diag(DiagId::sema_reference_var_requires_init, loc).arg(name);
                self.emit(msg);
            }
            return;
        }
        if spec.storage() == VarStorage::Extern {
            return;
        }
        if !self.require_complete(ty, self.ast.decls.get(v).loc()) {
            let msg = if self.ast.types.is_void(ty) {
                self.diag(DiagId::sema_void_var, loc).arg(self.ty_arg(ty))
            } else {
                self.diag(DiagId::sema_typecheck_decl_incomplete_type, loc).arg(self.ty_arg(ty))
            };
            self.emit(self.incomplete_note(msg, ty));
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
            return;
        }
        if spec.is_constexpr() {
            let msg = self.diag(DiagId::sema_constexpr_var_non_constant, loc).arg(name);
            self.emit(msg);
            return;
        }
        let quals = self.ast.types.effective_quals(ty);
        let elem = self.base_element(ty);
        if self.element_class(elem).is_some() {
            // [dcl.init.general]/7–8, [class.dtor]/15 (WP-067a).
            self.check_class_default_init(v, ty, loc);
        } else if quals.is_const() {
            let msg = self.diag(DiagId::sema_default_init_const, loc).arg(self.ty_arg(ty));
            self.emit(msg);
        }
    }

    // -- function definitions ----------------------------------------------------

    /// Starts the definition of function `f` at `{`: checks for a
    /// redefinition, opens the function scope with the parameters.
    pub fn act_on_start_function_def(&mut self, f: DeclId) {
        // A deleted function is defined by its first declaration.
        let deleted = Some(self.ast.first(f)).filter(|&p| p != f && self.is_deleted_fn(p));
        if let Some(prev) = deleted.or_else(|| self.ast.definition(f))
            && prev != f
        {
            let msg = self
                .diag(DiagId::sema_redefinition, self.ast.decls.get(f).loc())
                .arg(self.decl_name(f));
            let msg = self.note_prev(msg, prev, true);
            self.emit(msg);
        }
        let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        let ret =
            self.ast.types.proto(fty).map_or(QualType::ERROR, kiln_ast::types::FuncProto::ret);
        let pushed = self.enter_definition_scopes(f);
        self.fn_saved.push(crate::class::SavedFn { func: self.func.take(), pushed });
        self.scopes.push(Scope::new(ScopeKind::Function, None));
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
            if !self.require_complete(pty, self.ast.decls.get(p).loc())
                && !self.ast.types.is_reference(pty)
                && !self.is_error_type(pty)
            {
                let msg = self
                    .diag(DiagId::sema_typecheck_decl_incomplete_type, self.ast.decls.get(p).loc())
                    .arg(self.ty_arg(pty));
                self.emit(self.incomplete_note(msg, pty));
            }
        }
        self.check_abstract_signature(f);
        self.begin_function_body(f, ret);
        if let Some(t) = self.method_this_type(f)
            && let Some(state) = self.func.as_mut()
        {
            state.this_ty = Some(t);
        }
    }

    /// Finishes the definition of `f` with `body`.
    pub fn act_on_finish_function_def(&mut self, f: DeclId, body: StmtId, rbrace: Loc) {
        self.end_function_body(body, rbrace);
        if self.ast.definition(f).is_none() {
            let _ = self.builder().set_body(f, body);
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
    pub fn elaborated_tag_type(&mut self, d: DeclId, keyword: TagKeyword) -> QualType {
        let t = self.tag_type(d);
        self.ast.types.elaborated(keyword, None, t)
    }

    /// Marks a type-name's declaration referenced ([basic.def.odr]: clang's
    /// `isReferenced` for types).
    pub fn mark_type_referenced(&mut self, d: DeclId) {
        self.ast.decls.set_flags(d, DeclFlags::REFERENCED);
        let first = self.ast.first(d);
        if first != d {
            self.ast.decls.set_flags(first, DeclFlags::REFERENCED);
        }
    }

    fn tag_kind_of(keyword: TagKeyword) -> TagKind {
        match keyword {
            TagKeyword::Class => TagKind::Class,
            TagKeyword::Union => TagKind::Union,
            _ => TagKind::Struct,
        }
    }

    fn tag_matches(&self, d: DeclId, keyword: TagKeyword) -> bool {
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
    ) -> Vec<DeclId> {
        match (qualifier, use_) {
            (None, _) if templ => self
                .lookup_current_scope(n)
                .into_iter()
                .filter(|&d| self.kind_of(d) == DeclKind::ClassTemplate)
                .filter_map(|d| self.pattern_of(d).map(|p| self.ast.latest(p)))
                .collect(),
            (Some(q), _) => self.lookup_qualified(q, n, LookupKind::Tag).decls,
            (None, TagUse::Reference) if !opaque_enum => self.lookup_name(n, LookupKind::Tag).decls,
            (None, _) => self
                .lookup_current_scope(n)
                .into_iter()
                .filter(|&d| is_tag_kind(self.ast.decls.get(d).kind()))
                .collect(),
        }
    }

    /// A class-key or `enum` followed by a name ([dcl.type.elab],
    /// [class.pre], [dcl.enum]). `scoped` is `EnumData::SCOPED[_STRUCT]`
    /// bits; `fixed` the enum-base. Returns the tag declaration.
    #[allow(clippy::too_many_arguments)]
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
        if use_ == TagUse::Friend {
            return self.act_on_friend_tag(keyword, name, qualifier, scoped, fixed, loc);
        }
        let is_enum = keyword == TagKeyword::Enum;
        let fixed = fixed.or(if is_enum && scoped != 0 { Some(QualType::INT) } else { None });
        if let Some(f) = fixed {
            let ok = self.ast.types.is_integral(f)
                || self.is_error_type(f)
                || self.ast.types.is_dependent(f);
            if !ok {
                let msg = self.diag(DiagId::sema_enum_invalid_underlying, loc).arg(self.ty_arg(f));
                self.emit(msg);
            }
        }
        let head = if !is_enum && matches!(use_, TagUse::Definition | TagUse::Declaration) {
            self.take_template_head(qualifier)
        } else {
            None
        };
        let mut prev = None;
        if let Some((n, nloc)) = name {
            if qualifier.is_none() && use_ != TagUse::Reference {
                self.check_template_param_shadow(n, nloc);
            }
            let opaque_enum = is_enum && fixed.is_some();
            let found = self.tag_redecl_lookup(n, qualifier, use_, head.is_some(), opaque_enum);
            if let Some(&p) = found.last() {
                if self.tag_matches(p, keyword) {
                    prev = Some(p);
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
                if use_ == TagUse::Reference {
                    self.mark_type_referenced(p);
                    return p;
                }
                if use_ == TagUse::Definition {
                    if let Some(def) = self.ast.definition(p) {
                        let msg = self.diag(DiagId::sema_redefinition, nloc).arg(sym_str(self, n));
                        let msg = self.note_prev(msg, def, true);
                        self.emit(msg);
                        prev = None;
                    }
                } else if use_ == TagUse::Declaration {
                    // A redeclaration creates no new decl (a deviation:
                    // clang adds a `ClassTemplateDecl` per redeclaration),
                    // but its template head is still checked against the
                    // template's ([temp.param]/12, /14).
                    if let Some(list) = self.templ.cur.take()
                        && let Some(t) = self.ast.described_template(p)
                    {
                        self.check_template_param_list(DeclKind::ClassTemplate, list, Some(t));
                    }
                    return p;
                }
            } else if qualifier.is_some() {
                let msg = self
                    .diag(DiagId::sema_no_member, nloc)
                    .arg(sym_str(self, n))
                    .arg(format!("'{}'", self.decl_name(qualifier.unwrap_or(self.ast.tu))));
                self.emit(msg);
            }
        }
        let ctx = qualifier.unwrap_or_else(|| self.tag_decl_context(use_));
        let dname = name.map_or(DeclName::EMPTY, |n| DeclName::ident(n.0));
        let mut h = DeclHead::new(ctx, dname, name.map_or(loc, |n| n.1));
        h.lexical = Some(if qualifier.is_some() { self.ctx } else { ctx });
        let d = if is_enum {
            let fx = fixed.map(|f| (f, self.enum_promotion(f)));
            self.builder().enumeration(h, scoped, fx, prev)
        } else {
            self.mark_pattern_next();
            self.builder().record(h, Self::tag_kind_of(keyword), prev)
        };
        if qualifier.is_none() {
            self.set_member_access(d);
        }
        if let Some(t) = self.finish_template_pattern(d)
            && qualifier.is_none()
        {
            self.set_member_access(t);
        }
        if let Some((n, _)) = name
            && self.in_local_scope()
            && qualifier.is_none()
        {
            self.push_local(n, d);
        }
        if let Some((n, _)) = name
            && qualifier.is_none()
            && prev.is_none()
        {
            self.check_using_shadow_conflict(d, n);
        }
        d
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
        let found = self.lookup_name(n, LookupKind::Tag).decls;
        let hidden = || {
            self.ast
                .lookup_local(ns, DeclName::ident(n))
                .iter()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .find(|&d| is_tag_kind(self.kind_of(d)))
        };
        let prev = found.last().copied().filter(|&p| within(self, p)).or_else(hidden);
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
        let d = self.builder().record(h, Self::tag_kind_of(keyword), None);
        self.friend_tag = Some(d);
        d
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
        self.classes.push(ClassBuild::new(r, tag));
        self.defining.push(r);
        self.scopes.push(Scope::new(ScopeKind::Record, Some(r)));
        self.ctx = r;
    }

    /// `}` of a class definition ([class.mem]/8: complete from here).
    /// `then_semi` says the `}` is followed by `;`: an unnamed class so
    /// followed is an anonymous struct/union ([class.union.anon]), which
    /// must be known before the definition's bits freeze.
    pub fn act_on_finish_record_definition(&mut self, r: DeclId, then_semi: bool) {
        self.scopes.pop();
        let build = if self.classes.last().is_some_and(|c| c.record == r) {
            self.classes.pop()
        } else {
            None
        };
        self.defining.retain(|&d| d != r);
        self.ctx = self
            .ast
            .decls
            .lexical_ctx(r)
            .or_else(|| self.ast.decls.get(r).ctx())
            .unwrap_or(self.ast.tu);
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
        let fields: Vec<DeclId> = self.ast.decls.members(r).collect();
        let mut any_named = false;
        for f in fields {
            if self.ast.decls.get(f).kind() != DeclKind::Field {
                continue;
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
            if !(fd.is_some_and(|x| x.bit_width.is_some())
                && self.ast.decls.get(f).name().is_empty())
            {
                any_named = true;
            }
        }
        if !any_named {
            bits = bits.with(DefinitionBits::EMPTY);
        }
        if self.ast.decls.record(r).is_some_and(|x| x.tag == TagKind::Union) {
            bits = bits.with(DefinitionBits::HAS_VARIANT_MEMBERS);
        }
        if then_semi && self.ast.decls.get(r).name() == DeclName::EMPTY {
            bits = bits.with(DefinitionBits::ANONYMOUS);
        }
        let bits = self.class_bits(r, build.as_ref(), bits);
        let bits = self.virtual_class_bits(r, bits);
        // Special members (spec/sema-class.md §4.1, §4.2, §4.4).
        let bits = self.special_member_bits(r, bits);
        let bits = self.finish_special_members(r, bits);
        if let Some(b) = &build {
            let _ = self.builder().set_friends(r, &b.friends);
        }
        self.builder().complete_record_definition(r, bits);
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
        self.enums.push(EnumBuild { decl: e, fixed, last: None, enumerators: Vec::new() });
        self.scopes.push(Scope::new(ScopeKind::Enum, Some(e)));
        self.ctx = e;
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
            let cty = if let Some(f) = build.fixed {
                // [dcl.enum]/5: a converted constant expression, so a
                // narrowing value is ill-formed ([expr.const]/12).
                if let Some(raw) = self.fold_int(c)
                    && !self.fits(f, raw)
                {
                    let msg = self
                        .diag(DiagId::sema_cce_narrowing_value, self.begin_loc(ie))
                        .arg("enumerator value")
                        .arg(raw.to_string())
                        .arg(self.ty_arg(f));
                    self.emit(msg);
                }
                c = conv::implicit_convert(self, c, f, ConvCtx::Initialization).unwrap_or(c);
                f
            } else {
                let cty = self.ast.exprs.get(c).ty();
                if self.ast.types.as_enum(cty).is_some() { QualType::INT } else { cty }
            };
            let c = self.act_on_full_expr(c);
            // A converted constant expression for a fixed underlying
            // type ([dcl.enum]/5), else an integral constant expression.
            let at = self.begin_loc(ie);
            let ctx = if build.fixed.is_some() {
                EvalContext::converted(at)
            } else {
                EvalContext::new(EvalKind::Other, at)
            };
            match self.eval_int(c, ctx) {
                Ok(v) => {
                    let v = self.check_enumerator_range(v, build.fixed, ie);
                    value = Some(v);
                    let cv = self.int_const(cty, v);
                    init_e = Some(self.builder().constant_expr(c, Some(cv)));
                }
                Err(f) => {
                    let msg = self.diag(DiagId::sema_expr_not_ice, self.ast.exprs.get(ie).loc());
                    let msg = Self::with_failure_notes(msg, f);
                    self.emit(msg);
                }
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
                let _ = self.builder().attach_attrs(d, attrs);
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
        if !attrs.is_empty() {
            let _ = self.builder().attach_attrs(d, attrs);
        }
        if !scoped && self.in_local_scope() {
            self.push_local(name, d);
        }
        if let Some(b) = self.enums.last_mut() {
            b.last = Some(v);
            b.enumerators.push((d, v));
        }
        d
    }

    /// The underlying and promotion types of an enumeration without a
    /// fixed underlying type ([dcl.enum]/7, [conv.prom]/3), after clang's
    /// `ActOnEnumBody`: the narrowest of `int`, `long`, `long long` when
    /// some value is negative, else of `unsigned int`, `unsigned long`,
    /// `unsigned long long`, promoting to the signed type of the same
    /// width unless the values need every bit.
    fn best_enum_types(&self, values: &[(DeclId, i128)]) -> (QualType, QualType) {
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
                    e.0 == name
                        && e.2 == crate::scope::Ns::Ordinary
                        && self.kind_of(e.1) != DeclKind::UsingShadow
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
            let (best, promotion) = self.best_enum_types(&build.enumerators);
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
    pub fn act_on_start_namespace(
        &mut self,
        name: Option<(Symbol, Loc)>,
        inline: bool,
        loc: Loc,
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
        } else {
            // [namespace.unnamed]/1: every unnamed namespace of a scope is
            // the same namespace.
            prev = self.ast.decls.members(self.ctx).find(|&m| {
                let n = self.ast.decls.get(m);
                n.kind() == DeclKind::Namespace && n.name() == DeclName::EMPTY
            });
        }
        let h = DeclHead::new(
            self.ctx,
            name.map_or(DeclName::EMPTY, |n| DeclName::ident(n.0)),
            name.map_or(loc, |n| n.1),
        );
        let ns = self.builder().namespace(h, inline, prev);
        if name.is_none() && prev.is_none() {
            // ... followed by an implicit `using namespace unique;`.
            let _ = self.using_directive(ns, loc, DeclFlags::IMPLICIT);
        }
        self.scopes.push(Scope::new(ScopeKind::Namespace, Some(ns)));
        self.ctx = ns;
        ns
    }

    /// `}` of a namespace.
    pub fn act_on_finish_namespace(&mut self, ns: DeclId) {
        self.scopes.pop();
        self.ctx = self.ast.decls.get(ns).ctx().unwrap_or(self.ast.tu);
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
                misc: kiln_ast::decl::DeclMisc::default(),
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
        }
        d
    }

    // -- static_assert and empty declarations ------------------------------------

    /// `static_assert(cond, msg);` ([dcl.pre]/10).
    pub fn act_on_static_assert(&mut self, cond: ExprId, msg: Option<ExprId>, loc: Loc) -> DeclId {
        let c = match conv::contextually_convert_to_bool(self, cond) {
            Ok(c) => c,
            Err(err) => {
                self.emit_conv_error(err, self.ast.exprs.get(cond).loc());
                cond
            }
        };
        let c = self.act_on_full_expr(c);
        let cloc = self.begin_loc(cond);
        // [dcl.pre]/10: a value-dependent condition waits for instantiation.
        if !self.has_errors_in(c) && !self.expr_is_value_dependent(c) {
            match self.eval_const(c, EvalContext::new(EvalKind::StaticAssert, loc)) {
                Ok(v) if self.const_truth(v) != Some(false) => {}
                Ok(_) => {
                    let text = msg.and_then(|m| self.string_literal_text(m));
                    // clang's `findFailedBooleanCondition`: the failing
                    // conjunct, printed unless it is a literal.
                    let inner = self.failed_condition(c);
                    let lit = matches!(
                        self.ast.exprs.get(inner).kind(),
                        ExprKind::CXXBoolLiteralExpr | ExprKind::IntegerLiteral
                    );
                    let d = if lit {
                        match text {
                            Some(t) => {
                                self.diag(DiagId::sema_static_assert_failed_msg, cloc).arg(t)
                            }
                            None => self.diag(DiagId::sema_static_assert_failed, cloc),
                        }
                    } else {
                        let mut req = String::new();
                        kiln_ast::types::TypeNames::write_expr(
                            &kiln_ast::dump::AstNames::new(&self.ast).canonical_expr_types(),
                            inner,
                            &mut req,
                        );
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
                    let d = self.explain_failed_comparison(d, inner);
                    self.emit(d);
                }
                Err(f) => {
                    let d = self.diag(DiagId::sema_static_assert_not_ice, cloc);
                    let d = Self::with_failure_notes(d, f);
                    self.emit(d);
                }
            }
        }
        let ctx = self.ctx;
        self.builder().static_assert(ctx, loc, c, msg)
    }

    /// The conjunct of the failed condition `c` that evaluates to false
    /// (clang's `findFailedBooleanCondition`), without parentheses and
    /// implicit conversions. Literal conjuncts are skipped.
    fn failed_condition(&mut self, c: ExprId) -> ExprId {
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
        self.ignore_paren_imp_casts(c)
    }

    fn conjunction_terms(&self, e: ExprId, out: &mut Vec<ExprId>) {
        let x = self.ignore_paren_imp_casts(e);
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
        let note = self
            .diag(DiagId::note_expr_evaluates_to, self.ast.exprs.get(c).loc())
            .arg(self.const_display(lv))
            .arg(op)
            .arg(self.const_display(rv));
        d.with_note(note)
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
                return Some(self.act_on_anonymous_record(ds, t));
            }
            return None;
        }
        if !ds.invalid {
            self.error(DiagId::sema_no_declarators, ds.loc);
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
        let ty = self.tag_type(rec);
        let h = DeclHead::new(ctx, DeclName::EMPTY, ds.loc).flags(DeclFlags::IMPLICIT);
        let obj = if in_class {
            self.builder().field(h, ty, None, 0)
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

    // -- name classification -----------------------------------------------------

    /// What `name` names in `X<...>::name` when the parser's lookahead
    /// skipped the template-id `X<...>` without forming the
    /// specialization: classified by the primary template's pattern (a
    /// partial or explicit specialization may differ; the parse proper
    /// looks up in the specialization).
    fn classify_template_member(&self, tmpl: DeclId, name: Symbol) -> NameClass {
        let Some(p) = self.ast.template_info(tmpl).and_then(|i| i.pattern) else {
            return NameClass::Dependent;
        };
        let Some(def) = self.ast.definition(p) else { return NameClass::Dependent };
        let found = self.lookup_qualified(def, name, LookupKind::Ordinary);
        let Some(&d) = found.decls.first() else { return NameClass::Dependent };
        let k = self.kind_of(self.ast.strip_shadow(d));
        match k {
            DeclKind::ClassTemplate | DeclKind::TypeAliasTemplate => NameClass::TypeTemplate(d),
            DeclKind::VarTemplate => NameClass::VarTemplate(d),
            DeclKind::FunctionTemplate => NameClass::FunctionTemplate(d),
            _ if is_type_kind(k) => NameClass::Type(QualType::DEPENDENT, d),
            _ => NameClass::Value(d),
        }
    }

    /// What an identifier names here (for parser decisions,
    /// [basic.lookup.unqual]); `qualifier` is the context named by a
    /// preceding nested-name-specifier.
    pub fn classify_name(&self, name: Symbol, qualifier: Option<DeclId>) -> NameClass {
        if let Some(q) = qualifier
            && self.kind_of(q) == DeclKind::ClassTemplate
        {
            return self.classify_template_member(q, name);
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
        let Some(&d) = found.decls.first() else { return NameClass::Undeclared };
        let k = self.ast.decls.get(d).kind();
        if k == DeclKind::Namespace {
            return NameClass::Namespace(d);
        }
        if k == DeclKind::NamespaceAlias {
            // [namespace.alias]/2: an alias names its namespace.
            return NameClass::Namespace(self.ast.resolve_alias(d));
        }
        // The injected-class-name of a class template's pattern names the
        // template ([temp.local]/1): `X` alone is `X<P...>`, `X<A>` a
        // template-id.
        if k == DeclKind::CXXRecord
            && let Some(t) = self.ast.described_template(self.ast.first(d))
        {
            return NameClass::TypeTemplate(t);
        }
        if is_type_kind(k) {
            let ty = if matches!(
                k,
                DeclKind::Typedef | DeclKind::TypeAlias | DeclKind::TemplateTypeParm
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
        let found = match prefix {
            Some(q) => self.lookup_qualified(q, name, LookupKind::Nested),
            None => self.lookup_name(name, LookupKind::Nested),
        };
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
        let ctx = match self.ast.decls.get(d).kind() {
            DeclKind::Namespace => Some(self.ast.first(d)),
            DeclKind::NamespaceAlias => Some(self.ast.resolve_alias(d)),
            // A class named only as a qualifier is not marked referenced
            // (clang's `isReferenced`).
            DeclKind::CXXRecord | DeclKind::Enum => Some(self.ast.definition(d).unwrap_or(d)),
            DeclKind::TemplateTypeParm | DeclKind::TemplateTemplateParm => Some(d),
            DeclKind::ClassTemplate | DeclKind::TypeAliasTemplate => {
                let what = if self.kind_of(d) == DeclKind::ClassTemplate {
                    "class template"
                } else {
                    "alias template"
                };
                let msg = self
                    .diag(DiagId::sema_template_requires_args, loc)
                    .arg(what)
                    .arg(format!("'{}'", sym_str(self, name)));
                self.emit(msg);
                None
            }
            DeclKind::Typedef | DeclKind::TypeAlias if self.is_dependent_ctx(d) => Some(d),
            DeclKind::Typedef | DeclKind::TypeAlias => {
                self.mark_type_referenced(d);
                let ty = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
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
        ctx.map(|c| (c, d))
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

    /// How a qualifier context is named in "no member named 'x' in ...".
    pub fn qualifier_desc(&self, q: DeclId) -> String {
        match self.ast.decls.get(q).kind() {
            DeclKind::Namespace => format!("namespace '{}'", self.qualified_decl_name(q)),
            DeclKind::TranslationUnit => "the global namespace".to_owned(),
            _ => {
                let t = self.tag_type(q);
                self.ty_arg(t)
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
        let scope_s = scope.map(|s| sym_str(self, s));
        let n = raw.trim_start_matches("__").trim_end_matches("__").to_owned();
        let gnu = syntax == AttrSyntax::Gnu || scope_s.as_deref() == Some("gnu");
        let std = scope_s.is_none() && syntax != AttrSyntax::Gnu;
        let clang_scope = scope_s.as_deref() == Some("clang");
        // clang 19 names only the attribute, not its scope.
        if scope_s.as_deref().is_some_and(|s| s != "gnu")
            && !(clang_scope && n == "using_if_exists")
        {
            let msg = self.diag(DiagId::parse_unknown_attribute, loc).arg(raw);
            self.emit(msg);
            return None;
        }
        let kind = match (n.as_str(), std, gnu) {
            ("nodiscard", true, _) | ("warn_unused_result", _, true) => AttrKind::NODISCARD,
            ("maybe_unused", true, _) => AttrKind::MAYBE_UNUSED,
            ("unused", _, true) => AttrKind::UNUSED,
            ("deprecated", _, _) => AttrKind::DEPRECATED,
            ("noreturn", _, _) => AttrKind::NORETURN,
            ("fallthrough", _, _) => AttrKind::FALLTHROUGH,
            ("likely", true, _) => AttrKind::LIKELY,
            ("unlikely", true, _) => AttrKind::UNLIKELY,
            ("no_unique_address", true, _) => AttrKind::NO_UNIQUE_ADDRESS,
            ("aligned", _, true) | ("alignas", _, _) => AttrKind::ALIGNED,
            ("packed", _, true) => AttrKind::PACKED,
            ("used", _, true) => AttrKind::USED,
            ("noinline", _, true) => AttrKind::NOINLINE,
            ("always_inline", _, true) => AttrKind::ALWAYS_INLINE,
            ("weak", _, true) => AttrKind::WEAK,
            ("const", _, true) => AttrKind::CONST,
            ("pure", _, true) => AttrKind::PURE,
            ("cold", _, true) => AttrKind::COLD,
            ("using_if_exists", _, true) => AttrKind::USING_IF_EXISTS,
            ("using_if_exists", _, _) if clang_scope => AttrKind::USING_IF_EXISTS,
            ("visibility" | "section" | "format", _, true) => {
                let msg = self.diag(DiagId::parse_attribute_ignored, loc).arg(raw);
                self.emit(msg);
                return None;
            }
            _ => {
                let msg = self.diag(DiagId::parse_unknown_attribute, loc).arg(raw);
                self.emit(msg);
                return None;
            }
        };
        let takes = match kind {
            AttrKind::NODISCARD | AttrKind::DEPRECATED => 1,
            AttrKind::ALIGNED => {
                usize::from(syntax != AttrSyntax::Keyword)
                    + usize::from(syntax == AttrSyntax::Keyword)
            }
            _ => 0,
        };
        if args.len() > takes {
            let msg = self
                .diag(DiagId::parse_attribute_wrong_arg_count, loc)
                .arg(n.clone())
                .arg(if takes == 0 { "0" } else { "1" });
            self.emit(msg);
            return None;
        }
        let (mut a, mut b) = (0u32, 0u32);
        match (kind, args.first()) {
            (AttrKind::ALIGNED, Some(AttrArg::Expr(e))) => {
                let e = self.check_alignment_expr(*e)?;
                a = e.raw();
            }
            (AttrKind::ALIGNED, Some(AttrArg::Type(t))) => b = t.to_raw(),
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
            (_, Some(AttrArg::Type(_))) => {
                self.error(DiagId::parse_expected_expression, loc);
                return None;
            }
            _ => {}
        }
        if kind == AttrKind::ALIGNED && args.is_empty() && syntax == AttrSyntax::Keyword {
            self.error(DiagId::parse_expected_expression, loc);
            return None;
        }
        let node = AttrNode { kind, syntax, flags: 0, loc, a, b };
        Some(self.ast.attrs.push(node))
    }

    /// An alignment argument ([dcl.align]/2): an integral constant
    /// expression whose value is a power of two (0 is ignored).
    fn check_alignment_expr(&mut self, e: ExprId) -> Option<ExprId> {
        if self.expr_is_value_dependent(e) {
            return Some(e);
        }
        let loc = self.ast.exprs.get(e).loc();
        let c = conv::lvalue_to_rvalue(self, e);
        let c = self.act_on_full_expr(c);
        let r = self.eval_int(c, EvalContext::new(EvalKind::Other, self.begin_loc(e)));
        if let Ok(v) = r {
            if v != 0 && (v < 0 || (v & (v - 1)) != 0) {
                self.error(DiagId::sema_alignment_not_power_of_two, loc);
                return None;
            }
            if v > 1 << 32 {
                let msg =
                    self.diag(DiagId::sema_alignment_too_big, loc).arg((1u64 << 32).to_string());
                self.emit(msg);
                return None;
            }
            let cv = self.int_const(self.ast.exprs.get(c).ty(), v);
            Some(self.builder().constant_expr(c, Some(cv)))
        } else if let Err(f) = r {
            let d = self.diag(DiagId::sema_expr_not_ice, loc);
            let d = Self::with_failure_notes(d, f);
            self.emit(d);
            None
        } else {
            None
        }
    }

    /// The attributes of a class-head or enum-head (`struct alignas(8)
    /// [[nodiscard]] S`, [class.pre]/1, [dcl.enum]/1).
    pub fn act_on_tag_attrs(&mut self, d: DeclId, attrs: &[AttrId]) {
        if attrs.is_empty() {
            return;
        }
        self.check_decl_attrs(d, attrs);
        let _ = self.builder().attach_attrs(d, attrs);
    }

    /// Checks attributes against the declaration they appertain to.
    pub(crate) fn check_decl_attrs(&mut self, d: DeclId, attrs: &[AttrId]) {
        let kind = self.ast.decls.get(d).kind();
        for &a in attrs {
            let node = *self.ast.attrs.get(a);
            let bad = match node.kind {
                AttrKind::NODISCARD => {
                    // Every function kind, constructors included
                    // ([dcl.attr.nodiscard]/1, /2 since P1771).
                    (!(kind.is_function() || matches!(kind, DeclKind::CXXRecord | DeclKind::Enum)))
                        .then_some("functions, classes, or enumerations")
                }
                AttrKind::NORETURN => {
                    (!kind.is_function()).then_some("functions, classes, or enumerations")
                }
                AttrKind::ALIGNED => {
                    if matches!(kind, DeclKind::Function | DeclKind::Typedef | DeclKind::TypeAlias)
                        && node.syntax == AttrSyntax::Keyword
                    {
                        Some("variables, data members and tag types")
                    } else if node.b != 0 || node.a != 0 {
                        self.check_underaligned(d, node);
                        None
                    } else {
                        None
                    }
                }
                AttrKind::PACKED => (kind != DeclKind::Field && kind != DeclKind::CXXRecord)
                    .then_some("structs, unions, classes and fields"),
                _ => None,
            };
            if let Some(what) = bad {
                let name = match node.kind {
                    AttrKind::NODISCARD => "'nodiscard'",
                    AttrKind::NORETURN => "'noreturn'",
                    AttrKind::PACKED => "'packed'",
                    _ => "'alignas'",
                };
                let msg =
                    self.diag(DiagId::sema_attribute_wrong_decl_type, node.loc).arg(name).arg(what);
                self.emit(msg);
            }
        }
    }

    /// `alignas` may not weaken alignment ([dcl.align]/5).
    fn check_underaligned(&mut self, d: DeclId, node: AttrNode) {
        if node.syntax != AttrSyntax::Keyword {
            return;
        }
        let Some(ty) = self.ast.decls.get(d).ty() else { return };
        let Some((_, natural)) = target::size_align(&self.ast, ty, &self.abi) else { return };
        let requested = if node.b != 0 {
            QualType::from_raw(node.b)
                .and_then(|t| target::size_align(&self.ast, t, &self.abi))
                .map(|x| x.1)
        } else {
            ExprId::from_raw(node.a)
                .and_then(|e| self.fold_int(e))
                .and_then(|v| u64::try_from(v).ok())
        };
        if let Some(r) = requested.filter(|&r| r != 0 && r < natural) {
            let _ = r;
            let msg = self
                .diag(DiagId::sema_alignas_underaligned, node.loc)
                .arg(natural.to_string())
                .arg(self.ty_arg(ty));
            self.emit(msg);
        }
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
