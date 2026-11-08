//! Itanium C++ name mangling ([abi:mangling]): functions, variables and
//! static data members in namespaces and classes, member functions with
//! cv- and ref-qualifiers, constructors and destructors (`C1`/`C2`,
//! `D0`/`D1`/`D2`), conversion functions, class, function and variable
//! template specializations (type, integral, enumeration, `nullptr`,
//! declaration and template template arguments, packs), local entities
//! (`Z...E`), member pointers, guard variables (`_ZGV`) and reference
//! temporaries (`_ZGR`), with substitutions as clang 19 makes them. See
//! spec/abi.md §"Mangling".

use kiln_ast::Ast;
use kiln_ast::RawId;
use kiln_ast::attr::AttrKind;
use kiln_ast::consts::{ByteImage, ConstValue, Member, PrintEnv, PtrBase, Scalar, Shape};
use kiln_ast::decl::{DeclKind, FnStorage, LinkageLang, NamespaceData, TemplLinkKind, VarStorage};
use kiln_ast::ids::{ArgListId, ConstValueId, DeclId, ExprId, QualType, TemplateNameId};
use kiln_ast::names::{DeclNameView, OverloadedOperator};
use kiln_ast::types::{ArgView, BuiltinType, RefQual, TemplateName, TypeView, VectorKind};
use kiln_source::fx::FxHashMap;

mod constraint;
mod dep;
mod reflect;

pub use reflect::ReflectionMangling;

/// The ids of unnamed classes that have no mangling number (`$_N`; clang
/// `getAnonymousStructId`), handed out in first-mangle order: per function
/// for a class whose context is a function, else from one module-wide
/// counter whose next id is the number of ids handed out so far (the
/// per-function ones included). One table per module, shared by every
/// symbol mangled for it (spec/abi.md §"Mangling").
#[derive(Default, Debug)]
pub struct AnonIds {
    ids: FxHashMap<DeclId, u32>,
    per_fn: FxHashMap<DeclId, u32>,
    /// Local-tag discriminators (built on first use): the index of each
    /// named class or enumeration directly in a function among those of
    /// the same name there, in declaration order.
    local_tags: Option<FxHashMap<DeclId, u32>>,
    /// Unnamed-type discriminators (built on first use): the index of
    /// each unnamed class or enumeration without a typedef name for
    /// linkage among those of its class or function, in declaration
    /// order.
    unnamed_tags: Option<FxHashMap<DeclId, u32>>,
    /// Whether the module has any `abi_tag` attribute (computed on first
    /// use): without one no name carries implicit tags.
    abi_tags: Option<bool>,
    /// Reflection literals written (`L <info> <reflection> E`,
    /// spec/reflection.md §8.3), over every symbol mangled for the module.
    refl_literals: u64,
    /// How reflections mangle (`-fkiln-reflection-mangling=`).
    refl_mode: ReflectionMangling,
}

impl AnonIds {
    /// An empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty table whose reflections mangle in form `mode`
    /// (spec/reflection.md §8.3).
    pub fn with_reflection_mangling(mode: ReflectionMangling) -> Self {
        AnonIds { refl_mode: mode, ..Self::default() }
    }

    /// The reflection mangling form of this table.
    pub fn reflection_mangling(&self) -> ReflectionMangling {
        self.refl_mode
    }

    /// How many reflection literals (`L <info> <reflection> E`) the
    /// symbols mangled with this table contain (spec/reflection.md §8.3),
    /// nested ones included.
    pub fn reflection_literals(&self) -> u64 {
        self.refl_literals
    }

    /// The id of class `d` (first declaration), assigning the next one
    /// (of function `func`'s counter, if any) on first use.
    fn id(&mut self, d: DeclId, func: Option<DeclId>) -> u32 {
        if let Some(&n) = self.ids.get(&d) {
            return n;
        }
        let n = match func {
            Some(f) => {
                let c = self.per_fn.entry(f).or_insert(0);
                *c += 1;
                *c - 1
            }
            None => u32::try_from(self.ids.len()).unwrap_or(u32::MAX),
        };
        self.ids.insert(d, n);
        n
    }

    /// The discriminator index of named local tag `d` (first
    /// declaration): how many tags of the same name its function declares
    /// before it (clang's mangling number less one,
    /// `ItaniumNumberingContext::getManglingNumber(const TagDecl *)`; the
    /// declaration order of an instantiated function follows its
    /// pattern's). One pass over the declarations builds the table.
    fn local_tag(&mut self, ast: &Ast, d: DeclId) -> u32 {
        let table = self.local_tags.get_or_insert_with(|| {
            let mut counts: FxHashMap<(DeclId, kiln_source::symbol::Symbol), u32> =
                FxHashMap::default();
            let mut t = FxHashMap::default();
            for x in ast.decls.ids() {
                if let Some((f, s)) = local_tag_key(ast, x) {
                    let c = counts.entry((f, s)).or_insert(0);
                    t.insert(x, *c);
                    *c += 1;
                }
            }
            t
        });
        table.get(&d).copied().unwrap_or(0)
    }

    /// The `<unnamed-type-name>` discriminator index of unnamed class or
    /// enumeration `d` (first declaration): how many unnamed tags without
    /// a typedef name for linkage (anonymous unions and structs included,
    /// closure types not) its class or function declares before it, in
    /// any nested block (clang's mangling number less one,
    /// `ItaniumNumberingContext::getManglingNumber(const TagDecl *)`
    /// keyed by the null identifier; `Sema::handleTagNumbering`). An
    /// instantiated class declares its members in its pattern's order.
    fn unnamed_tag(&mut self, ast: &Ast, d: DeclId) -> u32 {
        let table = self.unnamed_tags.get_or_insert_with(|| {
            let mut counts: FxHashMap<DeclId, u32> = FxHashMap::default();
            let mut t = FxHashMap::default();
            for x in ast.decls.ids() {
                if let Some(c) = unnamed_tag_ctx(ast, x) {
                    let c = counts.entry(c).or_insert(0);
                    t.insert(x, *c);
                    *c += 1;
                }
            }
            t
        });
        table.get(&ast.first(d)).copied().unwrap_or(0)
    }
}

/// The class or function an unnamed class or enumeration without a
/// typedef name for linkage is numbered under (`<unnamed-type-name>`);
/// `None` for other declarations, redeclarations and closure types.
fn unnamed_tag_ctx(ast: &Ast, d: DeclId) -> Option<DeclId> {
    let n = ast.decls.get(d);
    if !matches!(n.kind(), DeclKind::CXXRecord | DeclKind::Enum) || ast.first(d) != d {
        return None;
    }
    let c = n.ctx()?;
    let ck = ast.decls.get(c).kind();
    if !(is_function_kind(ck) || is_record_kind(ck)) {
        return None;
    }
    if !matches!(ast.names.view(n.name()), DeclNameView::Empty) {
        return None;
    }
    if ast.decls.typedef_for_anon(d).is_some() || ast.closure_lambda(d).is_some() {
        return None;
    }
    Some(c)
}

/// The (function, name) a named class or enumeration declared directly
/// in a function is numbered under; `None` for other declarations,
/// redeclarations, closure types and unnamed tags.
fn local_tag_key(ast: &Ast, d: DeclId) -> Option<(DeclId, kiln_source::symbol::Symbol)> {
    let n = ast.decls.get(d);
    if !matches!(n.kind(), DeclKind::CXXRecord | DeclKind::Enum) || ast.first(d) != d {
        return None;
    }
    let f = n.ctx().filter(|&c| is_function_kind(ast.decls.get(c).kind()))?;
    let DeclNameView::Identifier(s) = ast.names.view(n.name()) else {
        return None;
    };
    if ast.closure_lambda(d).is_some() {
        return None;
    }
    Some((f, s))
}

/// The semantic context of `d` as the mangler sees it (clang
/// `getEffectiveDeclContext`): a closure type created in a default
/// argument belongs to the parameter's function, and a namespace
/// definition that extends a namespace of the enclosing namespace's
/// inline namespace set ([namespace.def.general]/3) mangles under the
/// namespace it is written in, its lexical context, as clang does
/// (`namespace A { inline namespace I { namespace R {} } namespace R {
/// int y; } }` gives `_ZN1A1R1yE`; spec/sema.md §4.6), and a block-scope
/// `extern` declaration belongs to its namespace ([`local_extern_ns`]).
fn effective_ctx(ast: &Ast, d: DeclId) -> Option<DeclId> {
    if let Some(ns) = local_extern_ns(ast, d) {
        return Some(ns);
    }
    if ast.decls.get(d).kind() == DeclKind::Namespace
        && let Some(lex) = ast.decls.lexical_ctx(d)
    {
        return Some(lex);
    }
    if let Some(p) = closure_param_ctx(ast, d) {
        return ast.decls.get(p).ctx();
    }
    if let Some(c) = constrained_friend_class(ast, d) {
        return Some(c);
    }
    ast.decls.get(d).ctx()
}

/// The class of a member-like constrained friend ([temp.friend]/9,
/// clang `isMemberLikeConstrainedFriend`): a non-template friend
/// function with a requires-clause mangles in its class's scope, its
/// name prefixed by `F` (`_ZN1XIiEF2frES0_Q1CIT_E`).
fn constrained_friend_class(ast: &Ast, d: DeclId) -> Option<DeclId> {
    if !is_function_kind(ast.decls.get(d).kind()) || !ast.templates.trailing_requires.contains(d) {
        return None;
    }
    let lex = ast.decls.lexical_ctx(d)?;
    (is_record_kind(ast.decls.get(lex).kind()) && ast.decls.get(d).ctx() != Some(lex))
        .then_some(lex)
}

/// The parameter whose default argument created closure type `d`.
fn closure_param_ctx(ast: &Ast, d: DeclId) -> Option<DeclId> {
    let l = ast.closure_lambda(d)?;
    let c = ast.exprs.lambdas.get(l).mangling.context?;
    (ast.decls.get(c).kind() == DeclKind::ParmVar).then_some(c)
}

/// True if `r` is a closure type without a mangling number: it has no
/// linkage, and its members are internal.
fn is_unnumbered_closure(ast: &Ast, r: DeclId) -> bool {
    ast.closure_lambda(r).is_some_and(|l| ast.exprs.lambdas.get(l).mangling.number == 0)
}

/// A substitution candidate ([abi:mangling.compression]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sub {
    /// A namespace, class or enumeration (also as a type).
    Decl(DeclId),
    /// A template (as a template prefix or template template argument).
    Template(DeclId),
    /// A (canonical, possibly qualified) type.
    Type(QualType),
    /// A canonical identifier nested-name-specifier (`T::type::`).
    Nns(kiln_ast::ids::NnsId),
    /// A dependent template name `nns::template name` (canonical `nns`).
    DepTemplate(kiln_ast::ids::NnsId, kiln_source::symbol::Symbol),
    /// A template template parameter `(depth, index)`, as a template
    /// argument or as the template of a dependent specialization.
    TemplateParm(u32, u32),
    /// A <module-name> prefix (`W1AW1B`), by index into
    /// `Mangler::modnames`.
    Module(u32),
}

/// The constructor or destructor variant of a symbol ([abi:mangling]
/// <ctor-dtor-name>).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Structor {
    /// `C1` / `D1`: the complete-object constructor or destructor.
    Complete,
    /// `C2` / `D2`: the base-object constructor or destructor.
    Base,
    /// `D0`: the deleting destructor.
    Deleting,
}

/// The innermost enclosing namespace (or the translation unit) of a
/// block-scope function or `extern` variable declaration, which names
/// the namespace-scope entity ([basic.link]/6; clang
/// `mangleNameWithAbiTags` walks a local container out to its namespace
/// for an entity with linkage): `void f() { extern void g(int); }`
/// declares `_Z1gi`, not `_ZZ1fvE1gi`, also in an instantiation of a
/// function template.
pub fn local_extern_ns(ast: &Ast, d: DeclId) -> Option<DeclId> {
    let n = ast.decls.get(d);
    let extern_var = match n.kind() {
        DeclKind::Function => false,
        DeclKind::Var => true,
        _ => return None,
    };
    let c = n.ctx()?;
    if !is_function_kind(ast.decls.get(c).kind()) {
        return None;
    }
    if extern_var && ast.decls.var(d).is_none_or(|v| v.spec.storage() != VarStorage::Extern) {
        return None;
    }
    let mut c = Some(c);
    for _ in 0..10_000 {
        let ctx = c?;
        if matches!(ast.decls.get(ctx).kind(), DeclKind::Namespace | DeclKind::TranslationUnit) {
            return Some(ctx);
        }
        c = ast.decls.get(ctx).ctx();
    }
    None
}

/// True if the function or variable `d` has C language linkage
/// ([dcl.link]/4-5; clang `getDeclLanguageLinkage`, `isExternC`): its
/// first declaration is lexically inside an `extern "C"` linkage
/// specification, through enclosing functions and classes
/// ([`in_extern_c_context`]: a block-scope `extern` declaration, or a
/// friend declared in a class, in an `extern "C"` block), and it has
/// external linkage. A class member has C++ language linkage; an entity
/// with internal linkage (`extern "C" { static int x; }`, a `const`
/// variable in a braced specification) or a local variable has none,
/// and is mangled (`_ZL1x`).
pub fn is_extern_c(ast: &Ast, d: DeclId) -> bool {
    let first = ast.first(d);
    let n = ast.decls.get(first);
    let Some(c) = n.ctx() else { return false };
    let ck = ast.decls.get(c).kind();
    if is_record_kind(ck) {
        return false;
    }
    if is_function_kind(ck)
        && n.kind() != DeclKind::Function
        && ast.decls.var(first).is_none_or(|v| v.spec.storage() != VarStorage::Extern)
    {
        return false;
    }
    in_extern_c_context(ast, first) && !has_internal_linkage(ast, first)
}

/// True if `d` is lexically inside an `extern "C"` linkage
/// specification, the innermost one deciding, through any enclosing
/// namespaces, functions and classes (clang `isInExternCContext`).
pub fn in_extern_c_context(ast: &Ast, d: DeclId) -> bool {
    let mut c = ast.decls.lexical_ctx(d);
    for _ in 0..10_000 {
        let Some(ctx) = c else { return false };
        match ast.decls.get(ctx).kind() {
            DeclKind::LinkageSpec => {
                return ast
                    .decls
                    .scope(ctx)
                    .is_some_and(|s| s.linkage_lang() == Some(LinkageLang::C));
            }
            DeclKind::TranslationUnit => return false,
            _ => {}
        }
        c = ast.decls.lexical_ctx(ctx);
    }
    false
}

fn is_record_kind(k: DeclKind) -> bool {
    matches!(
        k,
        DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization
    )
}

fn is_function_kind(k: DeclKind) -> bool {
    matches!(
        k,
        DeclKind::Function
            | DeclKind::CXXMethod
            | DeclKind::CXXConstructor
            | DeclKind::CXXDestructor
            | DeclKind::CXXConversion
    )
}

/// True if `ns` is an unnamed namespace.
fn is_anon_ns(ast: &Ast, ns: DeclId) -> bool {
    ast.decls.namespace(ns).is_some_and(|n| n.flags & NamespaceData::ANONYMOUS != 0)
}

/// True if some enclosing namespace of `d` is unnamed.
fn in_anon_ns(ast: &Ast, d: DeclId) -> bool {
    let mut c = ast.decls.get(d).ctx();
    while let Some(ctx) = c {
        if ast.decls.get(ctx).kind() == DeclKind::Namespace && is_anon_ns(ast, ctx) {
            return true;
        }
        c = ast.decls.get(ctx).ctx();
    }
    false
}

/// True if the namespace-scope function or variable `d` has internal
/// linkage by its own declaration ([basic.link]/3): `static`, or a
/// non-`extern`/non-`inline` non-volatile `const` variable.
fn internal_by_storage(ast: &Ast, d: DeclId) -> bool {
    let first = ast.first(d);
    let n = ast.decls.get(first);
    if !is_file_context(ast, first) {
        return false;
    }
    // clang `internal_linkage` (a class's too: clang
    // `isInternalLinkageDecl` mangles it with `L`).
    if ast.has_internal_linkage_attr(first) {
        return true;
    }
    match n.kind() {
        DeclKind::Function => {
            ast.decls.function(first).is_some_and(|f| f.spec.storage() == FnStorage::Static)
        }
        DeclKind::Var | DeclKind::VarTemplateSpecialization => {
            let Some(v) = ast.decls.var(first) else { return false };
            if v.spec.storage() == VarStorage::Static {
                return true;
            }

            let q = ast.types.canonical(n.ty().unwrap_or(QualType::INT));
            // [basic.link]/3.2: only a non-volatile `const` type gives
            // internal linkage (`const volatile int cv = 1;` is external).
            // P2788: a `const` variable attached to a named module has
            // module (or, exported, external) linkage.
            const_non_volatile(ast, q)
                && !v.spec.is_inline()
                && (!ast.modules.active || ast.module_attachment(first).is_none())
                && v.spec.storage() != VarStorage::Extern
                && !ast.decls.is_single_line_linkage(first)
                && !matches!(v.templ.get(), Some((TemplLinkKind::Specialization, _)))
        }
        _ => false,
    }
}

/// True if `d` is emitted with internal linkage: it has internal linkage
/// ([`has_internal_linkage`]) or is unique-external (clang
/// `getLVForNamespaceScopeDecl`, `getLVForClassMember`: a variable whose
/// type, or a function whose type as written, has no linkage or internal
/// linkage, or a specialization whose template arguments do, [basic.link]/8;
/// such an entity keeps its unmangled or `_Z` name).
pub fn is_internal_symbol(ast: &Ast, d: DeclId) -> bool {
    has_internal_linkage(ast, d) || is_unique_external(ast, d)
}

fn is_unique_external(ast: &Ast, d: DeclId) -> bool {
    let first = ast.first(d);
    if is_extern_c(ast, first) {
        return false;
    }
    // A member of a class without linkage already has internal linkage
    // (`has_internal_linkage`); one of an externally visible class is
    // checked like a namespace-scope entity (clang `getLVForClassMember`).
    let member =
        ast.decls.get(first).ctx().is_some_and(|c| is_record_kind(ast.decls.get(c).kind()));
    if !member && !is_file_context(ast, first) {
        return false;
    }
    let n = ast.decls.get(first);
    let own = match n.kind() {
        DeclKind::TemplateParamObject => return !param_object_externally_visible(ast, first),
        DeclKind::Var => n.ty().is_some_and(|t| !type_externally_visible(ast, t)),
        k if is_function_kind(k) => {
            n.ty().is_some_and(|t| !fn_type_externally_visible(ast, first, t))
        }
        _ => return false,
    };
    // The template arguments of a function or variable template
    // specialization (clang `mergeTemplateLV`); a member of a class
    // template specialization takes its class's (`tag_externally_visible`).
    own || ast.spec_info(first).is_some_and(|(_, s)| !args_externally_visible(ast, s.args))
}

/// clang's linkage of function `f`'s type as written: its parameter
/// types and, unless deduced from a placeholder, its return type.
fn fn_type_externally_visible(ast: &Ast, f: DeclId, ty: QualType) -> bool {
    let TypeView::Function(p) = ast.types.canonical_view(ast.types.canonical(ty)) else {
        return true;
    };
    let deduced =
        ast.redecls_of(f).any(|r| ast.decls.function(r).is_some_and(|x| x.spec.return_deduced()));
    (deduced || type_externally_visible(ast, p.ret()))
        && ast.types.params(&p).iter().all(|&x| type_externally_visible(ast, x))
}

/// clang `getLVForTemplateArgumentList` is externally visible: every
/// type argument's type, every entity a non-type argument names, and
/// every template template argument.
fn args_externally_visible(ast: &Ast, list: ArgListId) -> bool {
    ast.types.args(list).iter().all(|a| match a.view() {
        ArgView::Type(q) => type_externally_visible(ast, q),
        ArgView::Value { ty, value } => {
            type_externally_visible(ast, ty) && value_externally_visible(ast, value)
        }
        ArgView::Template(n) | ArgView::TemplateExpansion { name: n, .. } => {
            match ast.types.template_name_view(ast.types.canonical_template_name(n)) {
                TemplateName::Template(t) => !has_internal_linkage(ast, t),
                _ => true,
            }
        }
        ArgView::Pack(l) => args_externally_visible(ast, l),
        ArgView::Null | ArgView::Expr { .. } | ArgView::Profile(_) => true,
    })
}

/// clang `getLVForValue` is externally visible: a pointer or reference
/// to, or a member pointer naming, an entity without external linkage is
/// not, and an aggregate is not when any of its pointer-like subobjects
/// is not — at any depth: members of members, array elements, base
/// subobjects and a union's active member (the frozen image keeps them
/// all as relocations, spec/ast.md "Constant values"). A pointer into a
/// template parameter object takes the object's linkage (its type and
/// value), one to a lifetime-extended temporary its extending
/// declaration's, `&typeid(T)` the linkage of `T`; any other object
/// without a declaration (a string literal, a frozen object) is internal,
/// as clang's expression bases are.
fn value_externally_visible(ast: &Ast, value: ConstValueId) -> bool {
    match ast.consts.get(value) {
        ConstValue::Scalar(Scalar::Pointer { base, .. }) => match base {
            PtrBase::Decl(d) | PtrBase::Function(d) => !is_internal_symbol(ast, d),
            PtrBase::Temporary(t) => {
                ast.decls.lifetime_extended(t).is_none_or(|l| !is_internal_symbol(ast, l.extending))
            }
            PtrBase::TypeInfo(t) => type_externally_visible(ast, t),
            PtrBase::StringLit { .. } | PtrBase::Const(_) | PtrBase::Label { .. } => false,
        },
        ConstValue::Scalar(Scalar::MemberPtr { member, .. }) => !is_internal_symbol(ast, member),
        // A reflection mangles as its reflectee (§8.3 of
        // spec/reflection.md) and has the reflectee's linkage.
        ConstValue::Scalar(Scalar::Reflection { id, .. }) => {
            reflect::reflection_externally_visible(ast, id)
        }
        ConstValue::Scalar(_) => true,
        ConstValue::Aggregate(img) => {
            img.relocs.iter().all(|r| value_externally_visible(ast, r.value))
        }
    }
}

/// clang's linkage of a template parameter object (`getLVForDecl` of a
/// `TemplateParamObjectDecl`): that of its type merged with its value's.
fn param_object_externally_visible(ast: &Ast, d: DeclId) -> bool {
    ast.decls.get(d).ty().is_none_or(|t| type_externally_visible(ast, t))
        && ast.templates.param_object_value(d).is_none_or(|v| value_externally_visible(ast, v))
}

/// clang `Type::getLinkage` is externally visible, for the types kiln
/// distinguishes: a type built from a class or enumeration without linkage
/// (an unnamed one without a typedef name for linkage, a local one outside
/// an inline function, one in an unnamed namespace, a closure type
/// without a mangling number, or a class template specialization with
/// such a template argument) has none.
pub fn type_externally_visible(ast: &Ast, q: QualType) -> bool {
    let t = ast.types.canonical(q);
    match ast.types.canonical_view(t) {
        TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => {
            type_externally_visible(ast, p)
        }
        TypeView::ConstantArray(_) | TypeView::IncompleteArray(_) => {
            ast.types.element(t).is_none_or(|e| type_externally_visible(ast, e))
        }
        TypeView::MemberPointer { pointee, class } => {
            type_externally_visible(ast, pointee)
                && type_externally_visible(ast, QualType::unqual(class))
        }
        TypeView::Function(p) => {
            type_externally_visible(ast, p.ret())
                && ast.types.params(&p).iter().all(|&x| type_externally_visible(ast, x))
        }
        TypeView::Record(d) | TypeView::Enum(d) => tag_externally_visible(ast, ast.first(d)),
        _ => true,
    }
}

fn tag_externally_visible(ast: &Ast, d: DeclId) -> bool {
    if ast.closure_lambda(d).is_some() {
        if is_unnumbered_closure(ast, d) {
            return false;
        }
    } else if decl_name(ast, d).is_empty()
        && ast.decls.typedef_for_anon(d).is_none()
        && effective_ctx(ast, d).is_none_or(|c| {
            let k = ast.decls.get(c).kind();
            !is_function_kind(k) && !is_record_kind(k)
        })
    {
        // Only a namespace-scope unnamed class lacks linkage outright
        // (clang `getLVForNamespaceScopeDecl` `hasNameForLinkage`); a
        // member or local one takes its enclosing class's or function's
        // (`getLVForClassMember`, `getLVForLocalDecl`).
        return false;
    }
    if in_anon_ns(ast, d) || ast.has_internal_linkage_attr(d) {
        return false;
    }
    // A class template specialization takes the linkage of its template
    // arguments (clang `getLVForDecl` → `mergeTemplateLV`).
    if let Some((_, s)) = ast.spec_info(d)
        && !args_externally_visible(ast, s.args)
    {
        return false;
    }
    match effective_ctx(ast, d) {
        Some(c) if is_function_kind(ast.decls.get(c).kind()) => {
            fn_shares_local_linkage(ast, c) && !is_internal_symbol(ast, c)
        }
        Some(c) if is_record_kind(ast.decls.get(c).kind()) => {
            tag_externally_visible(ast, ast.first(c))
        }
        _ => true,
    }
}

/// True if `d`'s semantic context is a namespace (or the translation
/// unit), looking through linkage specifications.
fn is_file_context(ast: &Ast, d: DeclId) -> bool {
    let mut c = ast.decls.get(d).ctx();
    while let Some(ctx) = c {
        match ast.decls.get(ctx).kind() {
            DeclKind::TranslationUnit | DeclKind::Namespace => return true,
            DeclKind::LinkageSpec | DeclKind::Export => c = ast.decls.get(ctx).ctx(),
            _ => return false,
        }
    }
    true
}

/// True if `d` (a function, member function, variable or static data
/// member) has internal linkage ([basic.link]/4): `static` or `const` at
/// namespace scope, a member of an unnamed namespace (or of a class in
/// one), or a member of a local class of a function with internal
/// linkage.
pub fn has_internal_linkage(ast: &Ast, d: DeclId) -> bool {
    let first = ast.first(d);
    if internal_by_storage(ast, first) || ast.has_internal_linkage_attr(first) {
        return true;
    }
    // A namespace-scope function or variable of an unnamed namespace
    // first declared in an `extern "C"` context keeps external linkage
    // (clang `getLVForNamespaceScopeDecl`).
    if in_anon_ns(ast, first)
        && !(matches!(ast.decls.get(first).kind(), DeclKind::Function | DeclKind::Var)
            && is_file_context(ast, first)
            && in_extern_c_context(ast, first))
    {
        return true;
    }
    // A local entity has no linkage unless its function is inline or a
    // template instantiation (then it takes the function's); a member of a class without linkage (clang
    // `isExternallyVisible`: an unnamed or local class, a closure type
    // without a mangling number) has none either.
    let mut c = effective_ctx(ast, first);
    while let Some(ctx) = c {
        let k = ast.decls.get(ctx).kind();
        if is_function_kind(k) {
            // The function's linkage includes its template arguments':
            // `call<(anonymous)::A>`'s lambda is internal too (clang
            // `getLVForLocalDecl` takes `getLVForDecl(FD)`).
            return !fn_shares_local_linkage(ast, ctx) || is_internal_symbol(ast, ctx);
        }
        if !is_record_kind(k) {
            break;
        }
        if !tag_externally_visible(ast, ast.first(ctx)) {
            return true;
        }
        c = effective_ctx(ast, ctx);
    }
    false
}

/// True if class `rd` is not externally visible: it has internal linkage
/// or no linkage (clang `getTypeInfoLinkage`, `getVTableLinkage`).
pub fn class_is_internal(ast: &Ast, rd: DeclId) -> bool {
    let first = ast.first(rd);
    has_internal_linkage(ast, first) || !tag_externally_visible(ast, first)
}

/// True if the local entities of function `f` take its linkage (clang
/// `getLVForLocalDecl`): `f` is inline or a template instantiation (an
/// implicit or explicit instantiation of a function template, or a
/// member function of a class template specialization that is not
/// explicitly specialized).
fn fn_shares_local_linkage(ast: &Ast, f: DeclId) -> bool {
    fn_is_inline(ast, f) || fn_is_template_instantiation(ast, f)
}

/// True if `f` is an instantiated function (clang
/// `isTemplateInstantiation(getTemplateSpecializationKind())`).
fn fn_is_template_instantiation(ast: &Ast, f: DeclId) -> bool {
    use kiln_ast::templates::SpecKind;
    let first = ast.first(f);
    match ast.templ_link(first).get() {
        Some((TemplLinkKind::MemberOfSpecialization, i)) => {
            ast.templates.member_spec(i).is_some_and(|m| m.kind != SpecKind::ExplicitSpecialization)
        }
        Some((TemplLinkKind::Specialization, _)) => ast.spec_info(first).is_some_and(|(_, s)| {
            matches!(
                s.kind,
                SpecKind::ImplicitInstantiation
                    | SpecKind::ExplicitInstantiationDeclaration
                    | SpecKind::ExplicitInstantiationDefinition
            )
        }),
        _ => false,
    }
}

/// An inline (or constexpr, or implicitly inline) function.
fn fn_is_inline(ast: &Ast, f: DeclId) -> bool {
    ast.redecls_of(f).any(|r| {
        ast.decls.function(r).is_some_and(|x| {
            x.spec.is_inline()
                || x.spec.implicit_inline()
                || x.spec.constexpr_kind() != kiln_ast::decl::ConstexprKind::None
        })
    })
}

/// Whether canonical type `q` is const-qualified and not volatile
/// ([basic.link]/3.2), the qualifiers of an array being those of its
/// element type ([basic.type.qualifier]/3).
fn const_non_volatile(ast: &Ast, q: QualType) -> bool {
    let mut q = q;
    let mut quals = ast.types.effective_quals(q);
    while let Some(e) = ast.types.element(q) {
        q = ast.types.canonical(e);
        quals |= ast.types.effective_quals(q);
    }
    quals.is_const() && !quals.is_volatile()
}

/// The spelling of `d`'s name (empty for unnamed entities).
pub fn decl_name(ast: &Ast, d: DeclId) -> String {
    let n = ast.decls.get(d);
    if let DeclNameView::Identifier(s) = ast.names.view(n.name()) {
        ast.symbols().as_str(s).to_owned()
    } else {
        if let Some(td) = ast.decls.typedef_for_anon(d) {
            return decl_name(ast, td);
        }
        ast.names.simple_spelling(n.name(), ast.symbols()).unwrap_or_default()
    }
}

/// True if `d` is a function whose type has the `regcall` convention
/// (only ever set on x86-64; clang `CC_X86RegCall`), whose name gets the
/// `__regcall3__` prefix (WP-808).
pub fn is_regcall_fn(ast: &Ast, d: DeclId) -> bool {
    // clang `getAsFunction`: a function template is its pattern.
    let d = match ast.decls.get(d).kind() {
        DeclKind::FunctionTemplate => match ast.template_info(d).and_then(|i| i.pattern) {
            Some(p) => p,
            None => return false,
        },
        _ => d,
    };
    let n = ast.decls.get(d);
    n.kind().is_function()
        && n.ty()
            .and_then(|t| ast.types.proto(ast.types.canonical(t)))
            .is_some_and(|p| p.info().cc() == kiln_ast::types::CallConv::RegCall)
}

/// True if the function or variable `d` is emitted under its plain
/// source name: `main`, `extern "C"` entities, and variables at global
/// scope.
pub fn is_unmangled(ast: &Ast, d: DeclId) -> bool {
    let n = ast.decls.get(d);
    if ast.decls.decomp_bindings(d).is_some() {
        return false;
    }
    // An `overloadable` function is always mangled (clang
    // `shouldMangleCXXName`), even with C language linkage.
    if is_extern_c(ast, d) && !ast.is_overloadable(d) {
        return true;
    }
    match n.kind() {
        DeclKind::Function => decl_name(ast, d) == "main" && in_global_ns(ast, d),
        // Only variables with internal linkage are mangled at global scope
        // (clang `shouldMangleCXXName`): `static int h;` is `_ZL1h`.
        // A variable template specialization is `_Z1vIiE`.
        // A variable with tags, its own or implicit from its type, is
        // mangled (clang `shouldHaveAbiTags`): `std::string s;` with
        // libstdc++ is `_Z1sB5cxx11`.
        DeclKind::Var => {
            in_global_ns(ast, d)
                && abi_tags(ast, d).is_none()
                && !is_local(ast, d)
                && !has_internal_linkage(ast, d)
                && ast.spec_info(ast.first(d)).is_none()
                && (!ast.modules.active || ast.module_attachment(ast.first(d)).is_none())
                && !var_type_has_tags(ast, d)
        }
        _ => false,
    }
}

/// Whether the type of variable `d` mentions an abi-tagged entity.
fn var_type_has_tags(ast: &Ast, d: DeclId) -> bool {
    let Some(ty) = ast.decls.get(d).ty() else { return false };
    if matches!(ast.types.canonical_view(ty), TypeView::Builtin(_)) {
        return false;
    }
    let mut anon = AnonIds::new();
    let mut m = Mangler::new(ast, &mut anon, "");
    !m.variable_type_tags(d).is_empty()
}

fn in_global_ns(ast: &Ast, d: DeclId) -> bool {
    let mut c = local_extern_ns(ast, d).or_else(|| ast.decls.get(d).ctx());
    while let Some(ctx) = c {
        match ast.decls.get(ctx).kind() {
            DeclKind::TranslationUnit => return true,
            DeclKind::LinkageSpec | DeclKind::Export => c = ast.decls.get(ctx).ctx(),
            _ => return false,
        }
    }
    true
}

fn is_local(ast: &Ast, d: DeclId) -> bool {
    enclosing_function(ast, d).is_some()
}

/// The function whose body declares `d` (directly or in a local class),
/// if any.
pub fn enclosing_function(ast: &Ast, d: DeclId) -> Option<DeclId> {
    let mut c = effective_ctx(ast, d);
    while let Some(ctx) = c {
        if is_function_kind(ast.decls.get(ctx).kind()) {
            return Some(ctx);
        }
        c = effective_ctx(ast, ctx);
    }
    None
}

/// The GNU asm label of `d`'s entity (recorded on its first declaration),
/// which replaces its linkage name. clang's `MangleContext::mangleName`
/// prefixes LLVM's `\x01` "no global prefix" marker when the target's
/// user-label prefix is non-empty (Mach-O's `_`); every kiln target is
/// ELF, whose prefix is empty, so the label is the symbol verbatim (a
/// `\x01` the user wrote stays: LLVM strips it when emitting).
///
/// An instantiated function or variable has the label of its pattern:
/// sema copies the pattern's `AsmLabel` attribute onto the
/// instantiation (clang `InstantiateAttrs`), so the attribute is read
/// when no label is recorded.
pub fn asm_label(ast: &Ast, d: DeclId) -> Option<String> {
    let f = ast.first(d);
    let l = match ast.decls.cold.asm_label.get(f) {
        Some(l) => *l,
        None if ast.instantiated_from(f).is_some() || ast.spec_info(f).is_some() => {
            let a = ast.redecls_of(f).find_map(|r| {
                crate::layout::props::attrs(ast, r).find(|a| a.kind == AttrKind::ASM_LABEL)
            })?;
            <kiln_ast::StringLitId as RawId>::from_raw(a.a)?
        }
        None => return None,
    };
    Some(String::from_utf8_lossy(ast.strings.bytes(l)).into_owned())
}

/// The `abi_tag` tags of `d` (sorted, unique, `,`-separated), from any
/// redeclaration or, for an instantiation, its pattern (clang instantiates
/// `AbiTagAttr` and clones it onto redeclarations). Once the walk crosses
/// a class-template member instantiation, a function pattern with a
/// definition contributes only that definition's tags: clang 19 takes the
/// instantiated member's attributes from the pattern definition, so an
/// out-of-line definition that does not repeat the tag drops it
/// (spec/abi.md §8.15).
fn abi_tags(ast: &Ast, d: DeclId) -> Option<&str> {
    let mut work = vec![(d, false)];
    let mut steps = 0;
    while let Some((cur, member)) = work.pop() {
        steps += 1;
        if steps > 16 {
            break;
        }
        let def_only =
            member && is_function_kind(ast.decls.get(cur).kind()) && ast.definition(cur).is_some();
        let tag_of = |r: DeclId| {
            crate::layout::props::attrs(ast, r)
                .find(|a| a.kind == AttrKind::ABI_TAG)
                .map(|a| ast.symbols().as_str(kiln_source::symbol::Symbol::from_index(a.a)))
        };
        let f = ast.first(cur);
        // An instantiated function carries its pattern's attributes
        // (sema `InstantiateAttrs`), but its tags are the pattern's as
        // found below, so its own are not consulted. That includes the
        // pattern of a member template instantiated with its class,
        // whose tags are those of the original member's definition.
        let member_tmpl = ast
            .described_template(f)
            .and_then(|t| ast.instantiated_from(t))
            .and_then(|t| ast.template_info(t))
            .and_then(|i| i.pattern);
        let instantiated = is_function_kind(ast.decls.get(cur).kind())
            && (ast.instantiated_from(f).is_some()
                || member_tmpl.is_some()
                || ast.spec_info(f).is_some_and(|(_, s)| {
                    s.kind != kiln_ast::templates::SpecKind::ExplicitSpecialization
                }));
        if instantiated {
        } else if def_only {
            if let Some(t) = ast.definition(cur).and_then(tag_of) {
                return Some(t);
            }
        } else if let Some(t) = ast.redecls_of(cur).find_map(tag_of) {
            return Some(t);
        }
        if let Some(p) = ast.template_info(f).and_then(|i| i.pattern) {
            work.push((p, member));
        }
        if let Some((_, s)) = ast.spec_info(f)
            && let Some(p) = ast.template_info(s.template).and_then(|i| i.pattern)
        {
            work.push((p, member));
        }
        if let Some(p) = ast.instantiated_from(f) {
            work.push((p, true));
        }
        if let Some(p) = member_tmpl {
            work.push((p, true));
        }
    }
    None
}

/// The linkage name of a function, variable or static data member (a
/// constructor or destructor gets its complete-object variant).
pub fn mangle_decl(ast: &Ast, anon: &mut AnonIds, d: DeclId) -> String {
    mangle_structor(ast, anon, d, Structor::Complete)
}

/// The linkage name of `d` with constructor/destructor variant `v`
/// (ignored for other functions).
pub fn mangle_structor(ast: &Ast, anon: &mut AnonIds, d: DeclId, v: Structor) -> String {
    if let Some(l) = asm_label(ast, d) {
        return l;
    }
    if is_unmangled(ast, d) {
        // clang `getMangledNameImpl`: an unmangled `regcall` function is
        // still prefixed `__regcall3__` (WP-808).
        if is_regcall_fn(ast, d) {
            return format!("__regcall3__{}", decl_name(ast, d));
        }
        return decl_name(ast, d);
    }
    let mut m = Mangler::new(ast, anon, "_Z");
    m.encoding(d, v);
    m.out
}

/// The name of the comdat shared by the variants of structor `d`
/// (Itanium 5.1.4.4 `C5`/`D5`, clang `mangleCXXCtorComdat` /
/// `mangleCXXDtorComdat`): the base-object mangling with `5` for `2`.
pub fn mangle_structor_comdat(ast: &Ast, anon: &mut AnonIds, d: DeclId) -> String {
    let mut m = Mangler::new(ast, anon, "_Z");
    m.structors = StructorSpelling::Comdat;
    m.encoding(d, Structor::Base);
    m.out
}

/// The linkage name of template parameter object `d` ([temp.param]/8):
/// `_ZTA` and its value as a template argument.
pub fn mangle_param_object(ast: &Ast, anon: &mut AnonIds, d: DeclId) -> String {
    let mut m = Mangler::new(ast, anon, "_ZTA");
    if let Some(v) = ast.templates.param_object_value(d) {
        let ty = ast.decls.get(d).ty().unwrap_or(QualType::INT);
        m.value_arg(ast.types.canonical(ty).without_quals(), v);
    }
    m.out
}

/// The linkage name of a static local `d`, the `index`-th (0-based)
/// static local of its function with the same name.
pub fn mangle_static_local(ast: &Ast, anon: &mut AnonIds, d: DeclId, index: u32) -> String {
    if let Some(l) = asm_label(ast, d) {
        return l;
    }
    let mut s = mangle_decl(ast, anon, d);
    push_discriminator(&mut s, index);
    s
}

/// <discriminator> := _ <number> (< 10) | __ <number> _ ; the first
/// occurrence has none, the second is number 0.
fn push_discriminator(s: &mut String, index: u32) {
    use std::fmt::Write as _;
    if let Some(n) = index.checked_sub(1) {
        if n < 10 {
            let _ = write!(s, "_{n}");
        } else {
            let _ = write!(s, "__{n}_");
        }
    }
}

/// The guard variable of static-storage variable `d` (`_ZGV <name>`);
/// `index` is the static-local discriminator index (0 elsewhere).
pub fn mangle_guard(ast: &Ast, anon: &mut AnonIds, d: DeclId, index: u32) -> String {
    let name = mangle_static_local(ast, anon, d, index);
    match name.strip_prefix("_Z") {
        Some(rest) => format!("_ZGV{rest}"),
        None => format!("_ZGV{}{name}", name.len()),
    }
}

/// The thread-local wrapper (`_ZTW`) or initialization function
/// (`_ZTH`) of `thread_local` variable `d`: `prefix <name>` (clang
/// `mangleItaniumThreadLocalWrapper` / `mangleItaniumThreadLocalInit`;
/// Itanium ABI §3.4.3, [abi:mangling] <special-name>).
pub fn mangle_thread_local(ast: &Ast, anon: &mut AnonIds, d: DeclId, prefix: &str) -> String {
    let name = mangle_decl(ast, anon, d);
    match name.strip_prefix("_Z") {
        Some(rest) => format!("{prefix}{rest}"),
        None => format!("{prefix}{}{name}", name.len()),
    }
}

/// The initializer function of module `name` (partition `partition`):
/// `_ZGI <module-name>` (clang `mangleModuleInitializer`; a partition
/// adds `WP <source-name>`); defined by an interface or partition unit
/// and called by its importers (spec/modules.md §7).
pub fn mangle_module_initializer(ast: &Ast, name: &str, partition: Option<&str>) -> String {
    let mut anon = AnonIds::new();
    let mut m = Mangler::new(ast, &mut anon, "_ZGI");
    m.module_name(name);
    if let Some(p) = partition {
        m.out.push_str("WP");
        m.source_name(p);
    }
    m.out
}

/// The reference temporary `index` (clang's mangling number) extended by
/// variable `d`: `_ZGR <name> [<seq-id>] _` ([abi:mangling]
/// <special-name>).
pub fn mangle_ref_temporary(
    ast: &Ast,
    anon: &mut AnonIds,
    d: DeclId,
    local_index: u32,
    index: u32,
) -> String {
    let name = mangle_static_local(ast, anon, d, local_index);
    let mut s = match name.strip_prefix("_Z") {
        Some(rest) => format!("_ZGR{rest}"),
        None => format!("_ZGR{}{name}", name.len()),
    };
    if let Some(n) = index.checked_sub(1) {
        s.push_str(&base36(n as usize));
    }
    s.push('_');
    s
}

/// The <type> mangling of `q` on its own (no substitutions shared with
/// an enclosing symbol).
pub fn mangle_type(ast: &Ast, anon: &mut AnonIds, q: QualType) -> String {
    let mut m = Mangler::new(ast, anon, "");
    m.ty(q);
    m.out
}

/// `<special-name>` of a type: `prefix` (`_ZTI` / `_ZTS`) followed by its
/// <type> (WP-080).
pub fn mangle_rtti(ast: &Ast, anon: &mut AnonIds, q: QualType, prefix: &str) -> String {
    let mut m = Mangler::new(ast, anon, prefix);
    m.ty(q);
    m.out
}

/// `<special-name>` of a class: `prefix` followed by the class's <type>
/// (`_ZTV` vtable, `_ZTT` VTT, `_ZTI` `type_info`, `_ZTS` `type_info` name;
/// WP-080).
pub fn mangle_class_special(ast: &Ast, anon: &mut AnonIds, prefix: &str, class: DeclId) -> String {
    let mut m = Mangler::new(ast, anon, prefix);
    m.tag(class);
    m.out
}

/// The construction vtable of base `base` at byte `offset` in
/// `most_derived` (`_ZTC <type> <offset> _ <type>`, one substitution
/// table for both types, as clang's `mangleCXXCtorVTable`).
pub fn mangle_ctor_vtable(
    ast: &Ast,
    anon: &mut AnonIds,
    most_derived: DeclId,
    offset: i64,
    base: DeclId,
) -> String {
    use std::fmt::Write as _;
    let mut m = Mangler::new(ast, anon, "_ZTC");
    m.tag(most_derived);
    let _ = write!(m.out, "{offset}_");
    m.tag(base);
    m.out
}

/// A thunk of `method` (variant `v` for a destructor): `this` adjustment
/// `this` = (non-virtual, vcall offset offset) and return adjustment
/// `ret` = (non-virtual, vbase offset offset) (Itanium §5.1.4
/// <special-name> `T <call-offset> <encoding>` / `Tc <call-offset>
/// <call-offset> <encoding>`).
pub fn mangle_thunk(
    ast: &Ast,
    anon: &mut AnonIds,
    method: DeclId,
    v: Structor,
    this: (i64, i64),
    ret: (i64, i64),
) -> String {
    let mut m = Mangler::new(ast, anon, "_ZT");
    let covariant = ret != (0, 0);
    if covariant {
        m.out.push('c');
    }
    push_call_offset(&mut m.out, this);
    if covariant {
        push_call_offset(&mut m.out, ret);
    }
    m.encoding(method, v);
    m.out
}

/// <call-offset> ::= h <nv-offset> _ | v <offset number> _ <virtual offset number> _
fn push_call_offset(s: &mut String, (nv, virt): (i64, i64)) {
    use std::fmt::Write as _;
    let num = |n: i64| if n < 0 { format!("n{}", n.unsigned_abs()) } else { n.to_string() };
    if virt == 0 {
        let _ = write!(s, "h{}_", num(nv));
    } else {
        let _ = write!(s, "v{}_{}_", num(nv), num(virt));
    }
}

/// The Itanium mangler state for one symbol.
struct Mangler<'a, 'b> {
    ast: &'a Ast,
    anon: &'b mut AnonIds,
    out: String,
    subs: Vec<Sub>,
    /// The module-name prefixes seen (`A`, `A.B`), for [`Sub::Module`].
    modnames: Vec<&'a str>,
    /// How constructor and destructor variants are spelled.
    structors: StructorSpelling,
    /// Template parameters mangle with their declared depth (`TL0__`):
    /// inside type-constraints and requires-clauses (constraint.rs).
    real_depth: bool,
    /// Comparing template-heads (constraint.rs): depths are ignored.
    head_only: bool,
    /// clang's `FunctionTypeDepth`: the number of function types being
    /// mangled (the encoded function's bare function type, a
    /// requires-clause and a requires-expression's parameter list
    /// included) and whether the innermost one's result type is (`fp`/`fL`
    /// nesting of a parameter reference, [abi] <function-param>).
    fn_depth: (u32, bool),
    /// clang's `AbiTagState`: every ABI tag mangled so far, explicitly
    /// (`B <source-name>`) or implicitly (an abi-tagged namespace in a
    /// prefix), in mangling order with repeats.
    used_tags: Vec<&'a str>,
    /// The tags written as `B <source-name>` so far.
    emitted_tags: Vec<&'a str>,
    /// The number of [`Mangler::trial_tags`] runs in progress; clang's
    /// `DisableDerivedAbiTags` while non-zero: collecting the tags of a
    /// return or variable type, or of a name, derives no implicit tags
    /// for the functions and variables mangled inside it.
    trials: u32,
    /// The implicit tags of the function or variable whose encoding is
    /// being mangled, written after the unqualified name of the
    /// declarations listed (the entity and, for a function template
    /// specialization, its template).
    extra_tags: Option<([DeclId; 2], Vec<&'a str>)>,
}

/// The structor-variant spelling of one [`Mangler`] run.
#[derive(Clone, Copy, PartialEq, Eq)]
enum StructorSpelling {
    /// The variant asked for (`C1`/`C2`, `D0`/`D1`/`D2`).
    Itanium,
    /// A structor comdat name: the base-object variant is `C5`/`D5`.
    Comdat,
    /// A constructor or destructor reflection (`fn <encoding>`) names the
    /// unified structor (`C4`/`D4`, as GCC).
    Unified,
}

/// The mangler state a trial mangling restores (clang copies the
/// outer mangler, substitutions included, into a throwaway one).
struct Snapshot {
    out: usize,
    subs: usize,
    modnames: usize,
    used: usize,
    emitted: usize,
    fn_depth: (u32, bool),
}

impl<'a, 'b> Mangler<'a, 'b> {
    /// A builtin's code. `__bf16` is the vendor type `u6__bf16` on
    /// `AArch64` (clang and GCC; not a substitution candidate there, unlike
    /// the SVE types) and `DF16b` on x86-64.
    fn builtin_str(&self, b: BuiltinType) -> &'static str {
        if b == BuiltinType::BFloat16
            && self.ast.types.target().long_double == kiln_ast::types::FloatFormat::IeeeQuad
        {
            "u6__bf16"
        } else if b == BuiltinType::MetaInfo && self.anon.refl_mode == ReflectionMangling::Gcc {
            // GCC's builtin `Dm` (not a substitution candidate).
            "Dm"
        } else {
            builtin_code(b)
        }
    }

    /// The source name of a Neon vector (WP-558): `AArch64` mangles it as
    /// its ACLE type `__<Elt>x<N>_t` (clang `mangleAArch64NeonVectorType`),
    /// other targets as `__simd64_<elt>` / `__simd128_<elt>` (clang
    /// `mangleNeonVectorType`).
    fn neon_vector_name(&self, elem: QualType, count: u32, poly: bool) -> String {
        use BuiltinType as B;
        let b = self.ast.types.builtin(elem);
        if self.ast.types.target().long_double == kiln_ast::types::FloatFormat::IeeeQuad {
            let base = match (poly, b) {
                (true, Some(B::UChar | B::SChar)) => "Poly8",
                (true, Some(B::UShort | B::Short)) => "Poly16",
                (true, _) => "Poly64",
                (false, Some(B::SChar)) => "Int8",
                (false, Some(B::Short)) => "Int16",
                (false, Some(B::Int)) => "Int32",
                (false, Some(B::Long | B::LongLong)) => "Int64",
                (false, Some(B::UChar)) => "Uint8",
                (false, Some(B::UShort)) => "Uint16",
                (false, Some(B::UInt)) => "Uint32",
                (false, Some(B::ULong | B::ULongLong)) => "Uint64",
                (false, Some(B::Half)) => "Float16",
                (false, Some(B::Double)) => "Float64",
                (false, Some(B::BFloat16)) => "Bfloat16",
                (false, _) => "Float32",
            };
            return format!("__{base}x{count}_t");
        }
        let elt = match (poly, b) {
            (true, Some(B::UChar | B::SChar)) => "poly8_t",
            (true, Some(B::UShort | B::Short)) => "poly16_t",
            (true, _) => "poly64_t",
            (false, Some(B::SChar)) => "int8_t",
            (false, Some(B::UChar)) => "uint8_t",
            (false, Some(B::Short)) => "int16_t",
            (false, Some(B::UShort)) => "uint16_t",
            (false, Some(B::Int)) => "int32_t",
            (false, Some(B::UInt)) => "uint32_t",
            (false, Some(B::Long | B::LongLong)) => "int64_t",
            (false, Some(B::ULong | B::ULongLong)) => "uint64_t",
            (false, Some(B::Double)) => "float64_t",
            (false, Some(B::Half)) => "float16_t",
            (false, Some(B::BFloat16)) => "bfloat16_t",
            (false, _) => "float32_t",
        };
        let esize: u32 = match b {
            Some(B::SChar | B::UChar | B::Char) => 1,
            Some(B::Short | B::UShort | B::Half | B::BFloat16) => 2,
            Some(B::Int | B::UInt | B::Float) => 4,
            _ => 8,
        };
        let bits = esize * 8 * count;
        format!("{}{elt}", if bits == 64 { "__simd64_" } else { "__simd128_" })
    }

    fn new(ast: &'a Ast, anon: &'b mut AnonIds, prefix: &str) -> Self {
        Mangler {
            ast,
            anon,
            out: String::from(prefix),
            subs: Vec::new(),
            modnames: Vec::new(),
            structors: StructorSpelling::Itanium,
            real_depth: false,
            head_only: false,
            fn_depth: (0, false),
            used_tags: Vec::new(),
            emitted_tags: Vec::new(),
            trials: 0,
            extra_tags: None,
        }
    }

    /// The semantic parent of `d`, looking through linkage
    /// specifications and `export`.
    fn parent(&self, d: DeclId) -> Option<DeclId> {
        // A concept template parameter's parent is the entity its
        // template-head templates (clang 22's DeclContext, P2841R7).
        if let Some(&o) = self.ast.templates.ttp_owner.get(d) {
            return Some(o);
        }
        let mut c = effective_ctx(self.ast, d);
        while let Some(ctx) = c {
            match self.ast.decls.get(ctx).kind() {
                DeclKind::LinkageSpec | DeclKind::Export => c = self.ast.decls.get(ctx).ctx(),
                _ => return Some(ctx),
            }
        }
        None
    }

    /// The template and arguments of `d` when it is a class, function or
    /// variable template specialization.
    fn spec_of(&self, d: DeclId) -> Option<(DeclId, ArgListId)> {
        let (_, s) = self.ast.spec_info(d)?;
        Some((s.template, s.args))
    }

    fn is_std(&self, ns: DeclId) -> bool {
        self.ast.decls.get(ns).kind() == DeclKind::Namespace
            && decl_name(self.ast, ns) == "std"
            && self
                .parent(ns)
                .is_none_or(|p| self.ast.decls.get(p).kind() == DeclKind::TranslationUnit)
    }

    /// The Itanium standard abbreviation of `d` ([abi:mangling.compression],
    /// clang `mangleStandardSubstitution`), never itself a substitution
    /// candidate: the class templates `::std::allocator` (`Sa`) and
    /// `::std::basic_string` (`Sb`), and the specializations
    /// `::std::basic_string<char, ::std::char_traits<char>,
    /// ::std::allocator<char>>` (`Ss`), `::std::basic_{i,o,io}stream<char,
    /// ::std::char_traits<char>>` (`Si`, `So`, `Sd`). Only entities declared
    /// directly in `::std` qualify: libstdc++'s `std::__cxx11::basic_string`
    /// is `NSt7__cxx1112basic_string...E`. A template attached to a named
    /// module gets none (clang's `getOwningModuleForLinkage` check):
    /// `StS_9allocatorIiE` in module `M`, not `SaIiE`.
    fn std_abbrev(&self, d: DeclId) -> Option<&'static str> {
        let ast = self.ast;
        if !self.parent(d).is_some_and(|p| self.is_std(p)) {
            return None;
        }
        let module_attached =
            |t: DeclId| ast.modules.active && ast.module_attachment(ast.first(t)).is_some();
        if ast.decls.get(d).kind() == DeclKind::ClassTemplate {
            if module_attached(d) {
                return None;
            }
            return match decl_name(ast, d).as_str() {
                "allocator" => Some("Sa"),
                "basic_string" => Some("Sb"),
                _ => None,
            };
        }
        let (t, args) = self.spec_of(d)?;
        if ast.decls.get(t).kind() != DeclKind::ClassTemplate || module_attached(t) {
            return None;
        }
        let (code, with_alloc) = match decl_name(ast, t).as_str() {
            "basic_string" => ("Ss", true),
            "basic_istream" => ("Si", false),
            "basic_ostream" => ("So", false),
            "basic_iostream" => ("Sd", false),
            _ => return None,
        };
        let args = ast.types.args(args);
        let ok = args.len() == if with_alloc { 3 } else { 2 }
            && self.is_char_arg(args[0].view())
            && self.is_std_char_spec(args[1].view(), "char_traits")
            && (!with_alloc || self.is_std_char_spec(args[2].view(), "allocator"));
        ok.then_some(code)
    }

    /// True if `a` is the type `char` (clang `isCharType`).
    fn is_char_arg(&self, a: ArgView) -> bool {
        let ArgView::Type(q) = a else { return false };
        matches!(
            self.ast.types.canonical_view(self.ast.types.canonical(q)),
            TypeView::Builtin(BuiltinType::Char)
        )
    }

    /// True if `a` is `::std::<name><char>` (clang `isCharSpecialization`).
    fn is_std_char_spec(&self, a: ArgView, name: &str) -> bool {
        let ArgView::Type(ty) = a else { return false };
        let Some(rec) = self.ast.types.as_record(ty) else { return false };
        let Some((tmpl, args)) = self.spec_of(rec) else { return false };
        self.parent(rec).is_some_and(|p| self.is_std(p))
            && !(self.ast.modules.active
                && self.ast.module_attachment(self.ast.first(tmpl)).is_some())
            && decl_name(self.ast, tmpl) == name
            && matches!(self.ast.types.args(args), [c] if self.is_char_arg(c.view()))
    }

    /// Writes `d`'s standard abbreviation, if it has one.
    fn try_std_abbrev(&mut self, d: DeclId) -> bool {
        match self.std_abbrev(d) {
            Some(code) => {
                self.out.push_str(code);
                true
            }
            None => false,
        }
    }

    // ---- encodings -----------------------------------------------------

    /// <encoding> ::= <name> <bare-function-type> | <name>
    fn encoding(&mut self, d: DeclId, v: Structor) {
        let ast = self.ast;
        let n = ast.decls.get(d);
        let is_fn = is_function_kind(n.kind());
        if is_fn
            && ((is_extern_c(ast, d) && !ast.is_overloadable(d))
                || (decl_name(ast, d) == "main" && in_global_ns(ast, d)))
        {
            // Unmangled functions appear in local names by source name only.
            self.source_name(&decl_name(ast, d));
            return;
        }
        let saved_extra = self.extra_tags.take();
        if is_fn {
            // Implicit tags: those of the return type that the name and
            // bare function type do not mention (clang
            // `mangleFunctionEncoding`, `makeFunctionReturnTypeTags`).
            let ret = self.return_type_tags(d);
            if !ret.is_empty() {
                let used = self.trial_tags(|m| m.function_encoding(d, v));
                self.set_extra_tags(d, &ret, &used);
            }
            self.function_encoding(d, v);
        } else {
            // A variable's implicit tags come from its type (clang
            // `mangleName(GlobalDecl)`, `makeVariableTypeTags`).
            let tys = self.variable_type_tags(d);
            if !tys.is_empty() {
                let used = self.trial_tags(|m| m.name(d, v));
                self.set_extra_tags(d, &tys, &used);
            }
            self.name(d, v);
        }
        self.extra_tags = saved_extra;
    }

    /// <name> <bare-function-type> of function `d`.
    fn function_encoding(&mut self, d: DeclId, v: Structor) {
        // The name is mangled inside the function's prototype scope:
        // its template arguments, template-head requires-clause and
        // lambda signatures may name its parameters one function type
        // deep (clang `mangleFunctionEncoding`).
        let saved = self.fn_depth;
        self.fn_depth = (saved.0 + 1, false);
        self.name(d, v);
        self.fn_depth = saved;
        self.enable_if_conds(d);
        self.bare_function_type(d);
    }

    /// `Ua9enable_ifI <template-arg>* E`: the conditions of the
    /// `enable_if` attributes of function `d` in declaration order, each
    /// mangled as a template argument inside the function's prototype
    /// scope, so overloads distinguished only by them get distinct names
    /// (clang `mangleFunctionEncodingBareType`; spec/sema-overload.md
    /// §4.3). The diagnostic message is not mangled.
    fn enable_if_conds(&mut self, d: DeclId) {
        let conds: Vec<ExprId> = crate::layout::props::attrs(self.ast, d)
            .filter(|a| a.kind == AttrKind::ENABLE_IF)
            .filter_map(|a| ExprId::from_raw(a.a))
            .collect();
        if conds.is_empty() {
            return;
        }
        let saved = self.fn_depth;
        self.fn_depth = (saved.0 + 1, false);
        self.out.push_str("Ua9enable_ifI");
        for c in conds {
            self.template_arg_expr(Some(c));
        }
        self.out.push('E');
        self.fn_depth = saved;
    }

    // ---- implicit ABI tags -----------------------------------------------

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            out: self.out.len(),
            subs: self.subs.len(),
            modnames: self.modnames.len(),
            used: self.used_tags.len(),
            emitted: self.emitted_tags.len(),
            fn_depth: self.fn_depth,
        }
    }

    fn restore(&mut self, s: &Snapshot) {
        self.out.truncate(s.out);
        self.subs.truncate(s.subs);
        self.modnames.truncate(s.modnames);
        self.used_tags.truncate(s.used);
        self.emitted_tags.truncate(s.emitted);
        self.fn_depth = s.fn_depth;
    }

    /// Runs `f` as a trial with derived tags disabled and returns the
    /// sorted, unique tags it used; the mangler's state is restored
    /// afterwards (clang mangles into a copy of itself whose
    /// `AbiTagsRoot` starts empty).
    fn trial_tags(&mut self, f: impl FnOnce(&mut Self)) -> Vec<&'a str> {
        let snap = self.snapshot();
        self.trials += 1;
        let extra = self.extra_tags.take();
        f(self);
        let mut tags = self.used_tags[snap.used..].to_vec();
        self.restore(&snap);
        self.trials -= 1;
        self.extra_tags = extra;
        tags.sort_unstable();
        tags.dedup();
        tags
    }

    /// The tags type `q` uses when mangled at this point (clang's
    /// throwaway `TrackReturnTypeTags` / `TrackVariableType` mangler).
    fn type_tags(&mut self, q: QualType, result: bool) -> Vec<&'a str> {
        self.trial_tags(|m| {
            if result {
                m.fn_depth = (m.fn_depth.0 + 1, true);
            }
            m.ty(q);
        })
    }

    /// Whether the module has any `abi_tag` attribute.
    fn has_abi_tags(&mut self) -> bool {
        let ast = self.ast;
        *self.anon.abi_tags.get_or_insert_with(|| ast.attrs.has_kind(AttrKind::ABI_TAG))
    }

    /// The tags of the return type of function `d` (none while derived
    /// tags are disabled, and none for an unmangled function).
    fn return_type_tags(&mut self, d: DeclId) -> Vec<&'a str> {
        let ast = self.ast;
        if self.trials > 0 || !self.has_abi_tags() {
            return Vec::new();
        }
        let Some(ty) = ast.decls.get(d).ty() else { return Vec::new() };
        let Some(ret) =
            ast.types.proto(ast.types.canonical(ty)).map(kiln_ast::types::FuncProto::ret)
        else {
            return Vec::new();
        };
        self.type_tags(ret, true)
    }

    /// The tags of the type of variable `d`.
    fn variable_type_tags(&mut self, d: DeclId) -> Vec<&'a str> {
        let ast = self.ast;
        let n = ast.decls.get(d);
        if self.trials > 0
            || n.kind() != DeclKind::Var
            || !self.has_abi_tags()
            || ast.decls.decomp_bindings(d).is_some()
        {
            return Vec::new();
        }
        let Some(ty) = n.ty() else { return Vec::new() };
        self.type_tags(ty, false)
    }

    /// Records the implicit tags of `d`: `derived` (sorted, unique) less
    /// `used`.
    fn set_extra_tags(&mut self, d: DeclId, derived: &[&'a str], used: &[&'a str]) {
        let extra: Vec<&'a str> =
            derived.iter().copied().filter(|t| used.binary_search(t).is_err()).collect();
        if !extra.is_empty() {
            let t = self.spec_of(d).map_or(d, |(t, _)| t);
            self.extra_tags = Some(([d, t], extra));
        }
    }

    /// <bare-function-type> of function `d`; a function template
    /// specialization mangles its template's signature with the return
    /// type first.
    fn bare_function_type(&mut self, d: DeclId) {
        let ast = self.ast;
        let kind = ast.decls.get(d).kind();
        // An inheriting constructor has the inherited constructor's
        // signature (clang `mangleFunctionEncoding`).
        let d = ast.decls.inherited_ctor(d).unwrap_or(d);
        let mut src = d;
        let mut with_ret = false;
        if let Some((t, _)) = self.spec_of(d) {
            if let Some(p) = ast.template_info(t).and_then(|i| i.pattern) {
                src = p;
            }
            with_ret = !matches!(
                kind,
                DeclKind::CXXConstructor | DeclKind::CXXDestructor | DeclKind::CXXConversion
            );
        }
        let Some(ty) = ast.decls.get(src).ty() else { return };
        let ty = ast.types.canonical(ty);
        let Some(p) = ast.types.proto(ty).copied() else { return };
        let saved = std::mem::replace(&mut self.fn_depth, (1, true));
        if with_ret {
            self.ty(p.ret());
        }
        self.fn_depth.1 = false;
        let params: Vec<QualType> = ast.types.params(&p).to_vec();
        if params.is_empty() && !p.variadic() {
            self.out.push('v');
        }
        // `pass_object_size(N)` is a vendor qualifier on the parameter
        // type (clang `mangleBareFunctionType`): `U17pass_object_size1`.
        let ps = ast.pass_object_size_params(src);
        for (i, q) in params.into_iter().enumerate() {
            self.ty(self.param_type(q));
            if let Some(&(_, t, dynamic)) = ps.iter().find(|p| p.0 == i) {
                let a = if dynamic { "pass_dynamic_object_size" } else { "pass_object_size" };
                self.out.push('U');
                self.out.push_str(&(a.len() + 1).to_string());
                self.out.push_str(a);
                self.out.push_str(&t.to_string());
            }
        }
        self.fn_depth = saved;
        if p.variadic() {
            self.out.push('z');
        }
        // A trailing requires-clause (clang 18+, cxx-abi issue 24): the
        // template pattern's, or an instantiated member's (uninstantiated)
        // copy.
        let trailing = ast
            .templates
            .trailing_requires
            .get(src)
            .or_else(|| ast.templates.trailing_requires.get(d));
        if let Some(&r) = trailing {
            self.requires_clause(r, true);
        }
    }

    /// The signature type of a parameter: top-level cv-qualifiers
    /// dropped.
    fn param_type(&self, q: QualType) -> QualType {
        let q = self.ast.types.canonical(q);
        if self.ast.types.element(q).is_some() {
            return q;
        }
        q.without_quals()
    }

    // ---- names ---------------------------------------------------------

    /// <name> of an entity.
    fn name(&mut self, d: DeclId, v: Structor) {
        let ast = self.ast;
        if let Some(f) = enclosing_function(ast, d) {
            // <local-name> := Z <function encoding> E <entity name>
            self.out.push('Z');
            let (used, emitted) = (self.used_tags.len(), self.emitted_tags.len());
            self.encoding(f, Structor::Complete);
            // Tags used implicitly (from namespaces) by the encoding are
            // not available to the entity; the written ones are (clang
            // `mangleLocalName`'s `LocalAbiTags`).
            let written = self.emitted_tags[emitted..].to_vec();
            self.used_tags.truncate(used);
            self.used_tags.extend(written);
            self.out.push('E');
            self.default_arg_discriminator(d, f);
            self.name_in(d, v, Some(f));
            // [<discriminator>] of the entity directly in `f` (clang
            // `mangleLocalName`): a named local class or enumeration
            // after the first of its name in `f`.
            let mut r = d;
            while let Some(c) = effective_ctx(ast, r)
                && c != f
            {
                r = c;
            }
            let r = ast.first(r);
            if local_tag_key(ast, r).is_some() {
                let i = self.anon.local_tag(ast, r);
                push_discriminator(&mut self.out, i);
            }
            return;
        }
        self.name_in(d, v, None);
    }

    /// `d [<number>] _` when local entity `d` of function `f` is (in) a
    /// closure type created in the default argument of `f`'s parameter
    /// counted `<number>` + 2 from the last (clang `mangleLocalName`).
    fn default_arg_discriminator(&mut self, d: DeclId, f: DeclId) {
        let ast = self.ast;
        let mut c = Some(d);
        while let Some(x) = c
            && x != f
        {
            if let Some(p) = closure_param_ctx(ast, x) {
                let params: Vec<DeclId> = ast
                    .decls
                    .function(f)
                    .map(|fd| ast.lists.iter::<DeclId>(fd.params).collect())
                    .unwrap_or_default();
                let idx = params.iter().position(|&q| q == p).unwrap_or(0);
                let num = params.len().saturating_sub(idx);
                self.out.push('d');
                if num > 1 {
                    self.out.push_str(&(num - 2).to_string());
                }
                self.out.push('_');
                return;
            }
            c = effective_ctx(ast, x);
        }
    }

    /// The variable or data member whose initializer created closure type
    /// `d`, which prefixes its name (`<closure-prefix>`, clang
    /// `getClosurePrefix`).
    fn closure_prefix_decl(&self, d: DeclId) -> Option<DeclId> {
        let l = self.ast.closure_lambda(d)?;
        let c = self.ast.exprs.lambdas.get(l).mangling.context?;
        matches!(self.ast.decls.get(c).kind(), DeclKind::Var | DeclKind::Field).then_some(c)
    }

    /// <closure-prefix> ::= [ <prefix> ] <unqualified-name> M
    ///                  ::= <template-prefix> <template-args> M, a
    /// substitution candidate.
    fn closure_prefix(&mut self, c: DeclId, stop: Option<DeclId>) {
        let key = Sub::Decl(self.ast.first(c));
        if self.try_sub(key) {
            return;
        }
        if let Some((t, args)) = self.spec_of(c) {
            // A variable template specialization: <template-prefix>
            // <template-args> (clang `mangleClosurePrefix`).
            self.template_prefix(t, stop, Structor::Complete, None);
            self.template_args(args);
        } else {
            if let Some(p) = self.parent(c) {
                self.prefix(p, stop);
            }
            self.unqualified(c, Structor::Complete);
        }
        self.out.push('M');
        self.subs.push(key);
    }

    /// The name of `d` relative to `stop` (the enclosing function of a
    /// local entity, or the translation unit).
    fn name_in(&mut self, d: DeclId, v: Structor, stop: Option<DeclId>) {
        if let Some(c) = self.closure_prefix_decl(d) {
            // A closure needs a nested name even at namespace scope.
            self.out.push('N');
            self.closure_prefix(c, stop);
            self.unqualified(d, v);
            self.out.push('E');
            return;
        }
        let parent = self.parent(d);
        let at_top = parent.is_none_or(|p| {
            Some(p) == stop || self.ast.decls.get(p).kind() == DeclKind::TranslationUnit
        });
        let spec = self.spec_of(d);
        if at_top || parent.is_some_and(|p| self.is_std(p)) {
            // <unscoped-name> / <unscoped-template-name> <template-args>
            let std = !at_top;
            if let Some((t, args)) = spec {
                self.unscoped_template(t, std, Some(d));
                self.template_args_of(t, args);
            } else {
                if std {
                    self.out.push_str("St");
                }
                self.l_prefix(d);
                self.unqualified(d, v);
            }
            return;
        }
        // <nested-name>
        self.out.push('N');
        self.method_quals(d);
        let p = parent.expect("nested name has a parent");
        if let Some((t, args)) = spec {
            self.template_prefix(t, stop, v, Some(d));
            self.template_args_of(t, args);
        } else {
            self.prefix(p, stop);
            self.l_prefix(d);
            self.unqualified(d, v);
        }
        self.out.push('E');
    }

    /// `L` for an internal-linkage name at namespace scope outside an
    /// unnamed namespace (clang `isInternalLinkageDecl`).
    fn l_prefix(&mut self, d: DeclId) {
        if internal_by_storage(self.ast, d)
            && !in_anon_ns(self.ast, d)
            && self.ast.decls.decomp_bindings(d).is_none()
            && anon_union_var_field(self.ast, d).is_none()
        {
            self.out.push('L');
        }
    }

    /// The cv- and ref-qualifiers (and `H` for an explicit object
    /// parameter) of a member function's nested name.
    fn method_quals(&mut self, d: DeclId) {
        let ast = self.ast;
        let n = ast.decls.get(d);
        if !matches!(n.kind(), DeclKind::CXXMethod | DeclKind::CXXConversion) {
            return;
        }
        if ast.decls.function(d).is_some_and(|f| f.spec.has_explicit_object()) {
            self.out.push('H');
            return;
        }
        let Some(p) = n.ty().and_then(|t| ast.types.proto(ast.types.canonical(t))) else { return };
        let info = p.info();
        let q = info.this_quals();
        if q.is_restrict() {
            self.out.push('r');
        }
        if q.is_volatile() {
            self.out.push('V');
        }
        if q.is_const() {
            self.out.push('K');
        }
        match info.ref_qual() {
            RefQual::LValue => self.out.push('R'),
            RefQual::RValue => self.out.push('O'),
            RefQual::None => {}
        }
    }

    /// <prefix> for context `dc` (a namespace or class), using and
    /// recording substitutions; nothing for the translation unit or
    /// `stop`.
    fn prefix(&mut self, dc: DeclId, stop: Option<DeclId>) {
        let ast = self.ast;
        let k = ast.decls.get(dc).kind();
        if Some(dc) == stop || k == DeclKind::TranslationUnit {
            return;
        }
        if matches!(k, DeclKind::LinkageSpec | DeclKind::Export) {
            if let Some(p) = self.parent(dc) {
                self.prefix(p, stop);
            }
            return;
        }
        if self.is_std(dc) {
            self.out.push_str("St");
            return;
        }
        if self.try_std_abbrev(dc) {
            return;
        }
        let key = Sub::Decl(ast.first(dc));
        if self.try_sub(key) {
            return;
        }
        if let Some((t, args)) = self.spec_of(dc) {
            self.template_prefix(t, stop, Structor::Complete, Some(dc));
            self.template_args(args);
        } else {
            if let Some(c) = self.closure_prefix_decl(dc) {
                self.closure_prefix(c, stop);
            } else if let Some(p) = self.parent(dc) {
                self.prefix(p, stop);
            }
            // A class with clang `internal_linkage`.
            self.l_prefix(dc);
            self.unqualified(dc, Structor::Complete);
        }
        self.subs.push(key);
    }

    /// <template-prefix> of template `t` within a nested name.
    /// `v` is the structor variant of a constructor template
    /// specialization (`C1`/`C2`); `spec` the specialization named,
    /// whose internal linkage at namespace scope gives the name its `L`.
    fn template_prefix(
        &mut self,
        t: DeclId,
        stop: Option<DeclId>,
        v: Structor,
        spec: Option<DeclId>,
    ) {
        if self.try_std_abbrev(t) {
            return;
        }
        let key = Sub::Template(self.ast.first(t));
        if self.try_sub(key) {
            return;
        }
        if let Some(p) = self.parent(t) {
            self.prefix(p, stop);
        }
        // A nested template name takes no implicit tags (clang's
        // `mangleTemplatePrefix` is not passed them; an unscoped one is).
        let extra = self.extra_tags.take();
        if let Some(spec) = spec {
            self.l_prefix(spec);
        }
        self.unqualified(t, v);
        self.extra_tags = extra;
        self.subs.push(key);
    }

    /// <unscoped-template-name> of template `t` (`St`-prefixed in `std`;
    /// `L`-prefixed when `spec`, the specialization named, has internal
    /// linkage, clang `isInternalLinkageDecl`).
    fn unscoped_template(&mut self, t: DeclId, std: bool, spec: Option<DeclId>) {
        if self.try_std_abbrev(t) {
            return;
        }
        let key = Sub::Template(self.ast.first(t));
        if self.try_sub(key) {
            return;
        }
        if std {
            self.out.push_str("St");
        }
        if let Some(spec) = spec {
            self.l_prefix(spec);
        }
        self.unqualified(t, Structor::Complete);
        self.subs.push(key);
    }

    /// <unqualified-name>: operator, constructor, destructor, conversion
    /// or literal-operator names, else the <source-name>.
    fn unqualified(&mut self, d: DeclId, v: Structor) {
        self.module_prefix(d);
        if constrained_friend_class(self.ast, d).is_some() {
            self.out.push('F');
        }
        self.unqualified_untagged(d, v);
        self.write_abi_tags(d);
    }

    /// <abi-tags> after the unqualified name of `d` (clang `writeAbiTags`,
    /// `AbiTagState::write`): its own tags plus the implicit ones of the
    /// entity being encoded, sorted. A namespace's tags are used but
    /// never written: `std::__cxx11` is `St7__cxx11`, and the tag reaches
    /// other names only implicitly, through a type that mentions it.
    fn write_abi_tags(&mut self, d: DeclId) {
        let ast = self.ast;
        if ast.decls.get(d).kind() == DeclKind::Namespace {
            // Only the first definition's tags count: a reopening's are
            // not merged (clang reads the canonical declaration).
            let own = crate::layout::props::attrs(ast, ast.first(d))
                .find(|a| a.kind == AttrKind::ABI_TAG)
                .map(|a| ast.symbols().as_str(kiln_source::symbol::Symbol::from_index(a.a)));
            self.used_tags.extend(own.into_iter().flat_map(|t| t.split(',')));
            return;
        }
        let own = abi_tags(ast, d);
        let extra = match &self.extra_tags {
            Some((ds, _)) if ds.contains(&d) => self.extra_tags.take().map(|(_, e)| e),
            _ => None,
        };
        if own.is_none() && extra.is_none() {
            return;
        }
        let mut list: Vec<&'a str> = own.into_iter().flat_map(|t| t.split(',')).collect();
        list.extend(extra.into_iter().flatten());
        list.sort_unstable();
        list.dedup();
        for t in list {
            self.used_tags.push(t);
            self.emitted_tags.push(t);
            self.out.push('B');
            self.source_name(t);
        }
    }

    /// `[<module-name>]` before the unqualified name of a namespace-scope
    /// entity with linkage attached to a named module (clang
    /// `mangleModuleName`; spec/modules.md §7). Partitions do not appear.
    fn module_prefix(&mut self, d: DeclId) {
        let ast = self.ast;
        if !ast.modules.active
            || !is_file_context(ast, d)
            || ast.closure_lambda(d).is_some()
            || ast.decls.get(d).kind() == DeclKind::Namespace
        {
            return;
        }
        let first = ast.first(d);
        let Some(m) = ast.module_attachment(first) else { return };
        if has_internal_linkage(ast, first) {
            return;
        }
        self.module_name(m);
    }

    /// <module-name> ::= <module-name>? W <source-name> | <substitution>;
    /// every prefix is a substitution candidate.
    fn module_name(&mut self, name: &'a str) {
        if let Some(i) = self.modnames.iter().position(|&n| n == name)
            && self.try_sub(Sub::Module(i as u32))
        {
            return;
        }
        let last = match name.rsplit_once('.') {
            Some((p, l)) => {
                self.module_name(p);
                l
            }
            None => name,
        };
        self.out.push('W');
        self.source_name(last);
        let i = self.modnames.iter().position(|&n| n == name).unwrap_or_else(|| {
            self.modnames.push(name);
            self.modnames.len() - 1
        });
        self.subs.push(Sub::Module(i as u32));
    }

    fn unqualified_untagged(&mut self, d: DeclId, v: Structor) {
        let ast = self.ast;
        let n = ast.decls.get(d);
        match ast.names.view(n.name()) {
            DeclNameView::Operator(op) => {
                let explicit = n
                    .ty()
                    .and_then(|t| ast.types.proto(ast.types.canonical(t)))
                    .map_or(0, |p| ast.types.params(p).len());
                // The implicit object parameter counts towards the arity
                // (static member operators are arity-independent).
                let implicit = n.kind() == DeclKind::CXXMethod
                    && ast.decls.function(d).is_some_and(|f| {
                        f.spec.storage() != FnStorage::Static && !f.spec.has_explicit_object()
                    });
                self.out.push_str(operator_code(op, explicit + usize::from(implicit)));
            }
            DeclNameView::Constructor(t) => {
                // An inheriting constructor ([class.inhctor.init]) is named
                // as the base constructor: `CI1`/`CI2` <base class name>.
                // clang `mangleCXXCtorType` mangles the base as a <name>,
                // not a <type>: the class itself is neither looked up in nor
                // added to the substitutions (only its prefixes are).
                if ast.decls.inheriting_ctor_shadow(d).is_some() {
                    self.out.push_str(match v {
                        Structor::Base if self.structors == StructorSpelling::Comdat => "CI5",
                        Structor::Base => "CI2",
                        _ => "CI1",
                    });
                    match ast.types.canonical_view(QualType::unqual(t)) {
                        TypeView::Record(base) => self.name(base, Structor::Complete),
                        _ => self.ty(QualType::unqual(t)),
                    }
                    // The inherited constructor template specialization's
                    // arguments follow (clang `mangleUnqualifiedName`'s
                    // `InheritedTemplateArgs`).
                    if let Some(bc) = ast.decls.inherited_ctor(d)
                        && let Some((_, args)) = self.spec_of(bc)
                    {
                        self.template_args(args);
                    }
                } else {
                    self.out.push_str(match v {
                        _ if self.structors == StructorSpelling::Unified => "C4",
                        Structor::Base if self.structors == StructorSpelling::Comdat => "C5",
                        Structor::Base => "C2",
                        _ => "C1",
                    });
                }
            }
            DeclNameView::Destructor(_) => self.out.push_str(match v {
                _ if self.structors == StructorSpelling::Unified => "D4",
                Structor::Complete => "D1",
                Structor::Base if self.structors == StructorSpelling::Comdat => "D5",
                Structor::Base => "D2",
                Structor::Deleting => "D0",
            }),
            DeclNameView::Conversion(q) => {
                self.out.push_str("cv");
                self.ty(q);
            }
            DeclNameView::LiteralOperator(s) => {
                self.out.push_str("li");
                let s = ast.symbols().as_str(s).to_owned();
                self.source_name(&s);
            }
            _ => {
                if n.kind() == DeclKind::Namespace && is_anon_ns(ast, d) {
                    self.source_name("_GLOBAL__N_1");
                    return;
                }
                let nm = decl_name(ast, d);
                if let Some(dc) = crate::mangle_decomp::decomposition_name(ast, d) {
                    self.out.push_str(&dc);
                } else if nm.is_empty()
                    && let Some(l) = ast.closure_lambda(d)
                {
                    self.closure_name(d, l);
                } else if nm.is_empty()
                    && let Some(f) = anon_union_var_field(ast, d)
                {
                    // A variable of anonymous union type is named by the
                    // union's first named data member (clang
                    // `mangleUnqualifiedName`, no `L`).
                    let fname = decl_name(ast, f);
                    self.source_name(&fname);
                } else if nm.is_empty() {
                    // An unnamed class or enumeration without a typedef
                    // name for linkage: `Ut [ <number> ] _`, the n-th
                    // (from 2) of its scope numbered `n - 2`.
                    match self.anon.unnamed_tag(ast, d) {
                        0 => self.out.push_str("Ut_"),
                        i => {
                            self.out.push_str("Ut");
                            self.out.push_str(&(i - 1).to_string());
                            self.out.push('_');
                        }
                    }
                } else if is_regcall_fn(ast, d) {
                    // clang `mangleUnqualifiedName`: a `regcall` function's
                    // identifier is prefixed `__regcall3__` (WP-808).
                    self.source_name(&format!("__regcall3__{nm}"));
                } else {
                    self.source_name(&nm);
                }
            }
        }
    }

    /// The name of closure type `d` of lambda `l`: `<closure-type-name> ::=
    /// Ul <lambda-sig> E [ <number> ] _` when it has a mangling number, else
    /// clang's `$_N` ([`AnonIds`]).
    fn closure_name(&mut self, d: DeclId, l: kiln_ast::ids::LambdaId) {
        let ast = self.ast;
        let data = ast.exprs.lambdas.get(l);
        let number = data.mangling.number;
        if number == 0 {
            let func = self.parent(d).filter(|&p| is_function_kind(ast.decls.get(p).kind()));
            let n = self.anon.id(ast.first(d), func);
            self.source_name(&format!("$_{n}"));
            return;
        }
        self.out.push_str("Ul");
        // A bare function type (clang `mangleLambdaSig`): one function
        // type deeper for the `fp`/`fL` nesting.
        let saved = self.fn_depth;
        self.fn_depth = (saved.0 + 1, false);
        let proto = data
            .call_op
            .and_then(|f| ast.decls.get(f).ty())
            .and_then(|t| ast.types.proto(ast.types.canonical(t)).copied());
        if let Some(p) = proto {
            let params: Vec<QualType> = ast.types.params(&p).to_vec();
            if params.is_empty() && !p.variadic() {
                self.out.push('v');
            }
            for q in params {
                self.ty(self.param_type(q));
            }
            if p.variadic() {
                self.out.push('z');
            }
        } else {
            self.out.push('v');
        }
        self.fn_depth = saved;
        self.out.push('E');
        if number > 1 {
            self.out.push_str(&(number - 2).to_string());
        }
        self.out.push('_');
    }

    fn source_name(&mut self, s: &str) {
        self.out.push_str(&s.len().to_string());
        self.out.push_str(s);
    }

    /// Emits a back-reference if `s` is a recorded candidate.
    fn try_sub(&mut self, s: Sub) -> bool {
        match self.subs.iter().position(|&x| x == s) {
            Some(k) => {
                self.sub_ref(k);
                true
            }
            None => false,
        }
    }

    fn sub_ref(&mut self, k: usize) {
        if k == 0 {
            self.out.push_str("S_");
        } else {
            self.out.push('S');
            self.out.push_str(&base36(k - 1));
            self.out.push('_');
        }
    }

    // ---- template arguments ---------------------------------------------

    /// The template arguments `list` of a specialization of `t`. A
    /// function template can be overloaded on its template-head, so an
    /// argument whose natural parameter differs from the actual one is
    /// prefixed by the parameter (clang 18+ `<template-param-decl>`,
    /// cxx-abi issue 47): `Tn <type>` for a non-type parameter of
    /// deduced or dependent type, `Tp` for a pack.
    fn template_args_of(&mut self, t: DeclId, list: ArgListId) {
        let ast = self.ast;
        let overloadable = ast.decls.get(t).kind() == DeclKind::FunctionTemplate
            && self
                .parent(t)
                .is_none_or(|c| ast.decls.record(c).is_none_or(|r| r.lambda.is_none()));
        let params: Vec<DeclId> = match ast.template_info(t) {
            Some(info) if overloadable => {
                let l = *ast.templates.param_list(info.params);
                ast.lists.iter::<DeclId>(l.params).collect()
            }
            _ => return self.template_args(list),
        };
        self.out.push('I');
        let args: Vec<_> = ast.types.args(list).to_vec();
        for (i, a) in args.into_iter().enumerate() {
            let a = a.view();
            let p = params.get(i).or_else(|| {
                params.last().filter(|&&p| ast.template_parm(p).is_some_and(|x| x.pack))
            });
            if let Some(&p) = p
                && self.needs_param_decl(p, a)
            {
                self.type_or_template_param_decl(p);
            }
            self.template_arg(a);
        }
        if let Some(r) = self.head_requires(t) {
            self.requires_clause(r, false);
        }
        self.out.push('E');
    }

    /// clang `needToMangleTemplateParam`.
    fn needs_param_decl(&mut self, p: DeclId, a: ArgView) -> bool {
        let ast = self.ast;
        match ast.decls.get(p).kind() {
            DeclKind::TemplateTypeParm => return self.is_constrained_type_parm(p),
            DeclKind::TemplateTemplateParm => return self.template_head_differs(p, a),
            DeclKind::NonTypeTemplateParm => {}
            _ => return false,
        }
        if let ArgView::Pack(l) = a {
            match ast.types.args(l).first() {
                None => return true,
                Some(x) => return self.needs_param_decl(p, x.view()),
            }
        }
        ast.decls.get(p).ty().is_some_and(|t| {
            ast.types.flags(t.ty()).intersects(
                kiln_ast::types::TypeFlags::DEPENDENT
                    | kiln_ast::types::TypeFlags::INSTANTIATION_DEPENDENT
                    | kiln_ast::types::TypeFlags::UNDEDUCED,
            )
        })
    }

    /// <template-param-decl> of non-type parameter `p`: `[Tp] Tn <type>`.
    fn template_param_decl(&mut self, p: DeclId) {
        let ast = self.ast;
        let mut ty = ast.decls.get(p).ty().unwrap_or(QualType::INT);
        if ast.template_parm(p).is_some_and(|x| x.pack) {
            self.out.push_str("Tp");
            if let TypeView::PackExpansion { pattern, .. } = ast.types.view(ty.ty()) {
                ty = pattern;
            }
        }
        self.out.push_str("Tn");
        self.ty(ty);
    }

    fn template_args(&mut self, list: ArgListId) {
        self.out.push('I');
        let args: Vec<_> = self.ast.types.args(list).to_vec();
        for a in args {
            self.template_arg(a.view());
        }
        self.out.push('E');
    }

    fn template_arg(&mut self, a: ArgView) {
        match a {
            ArgView::Type(q) => self.ty(q),
            ArgView::Value { ty, value } => self.value_arg(ty, value),
            ArgView::Template(n) | ArgView::TemplateExpansion { name: n, .. } => {
                if matches!(a, ArgView::TemplateExpansion { .. }) {
                    self.out.push_str("Dp");
                }
                self.template_name(n);
            }
            ArgView::Pack(l) => {
                self.out.push('J');
                let args: Vec<_> = self.ast.types.args(l).to_vec();
                for x in args {
                    self.template_arg(x.view());
                }
                self.out.push('E');
            }
            ArgView::Expr { expr, .. } => self.template_arg_expr(Some(expr)),
            ArgView::Profile(p) => self.template_arg_expr(self.ast.types.profile_expr(p)),
            ArgView::Null => self.out.push_str("XLi0EE"),
        }
    }

    /// A template template argument.
    fn template_name(&mut self, n: TemplateNameId) {
        let ast = self.ast;
        let n = ast.types.canonical_template_name(n);
        match ast.types.template_name_view(n) {
            TemplateName::Template(t) => {
                if self.try_std_abbrev(t) {
                    return;
                }
                let key = Sub::Template(ast.first(t));
                if self.try_sub(key) {
                    return;
                }
                let parent = self.parent(t);
                let top =
                    parent.is_none_or(|p| ast.decls.get(p).kind() == DeclKind::TranslationUnit);
                if top {
                    self.unqualified(t, Structor::Complete);
                } else if parent.is_some_and(|p| self.is_std(p)) {
                    self.out.push_str("St");
                    self.unqualified(t, Structor::Complete);
                } else {
                    self.out.push('N');
                    if let Some(p) = parent {
                        self.prefix(p, None);
                    }
                    self.unqualified(t, Structor::Complete);
                    self.out.push('E');
                }
                self.subs.push(key);
            }
            TemplateName::TemplateTemplateParm { depth, index, .. }
            | TemplateName::CanonTemplateTemplateParm { depth, index, .. } => {
                let key = Sub::TemplateParm(depth, index);
                if !self.try_sub(key) {
                    self.template_param_at(depth, index);
                    self.subs.push(key);
                }
            }
            _ => self.out.push_str("u7unknown"),
        }
    }

    /// A non-type template argument value: `L <type> <value> E`, or
    /// `L <mangled-name> E` for a reference to an entity.
    fn value_arg(&mut self, ty: QualType, value: ConstValueId) {
        self.value_in_arg(ty, value, true);
    }

    /// clang `mangleValueInTemplateArg`: value `value` of type `ty`; at
    /// the top level of a template argument (`top`) a non-primary
    /// expression is wrapped in `X…E`.
    fn value_in_arg(&mut self, ty: QualType, value: ConstValueId, top: bool) {
        let ty = self.value_type(ty);
        match self.ast.consts.get(value) {
            ConstValue::Aggregate(img) => self.image_value(&img, ty, 0, top),
            ConstValue::Scalar(s) => self.scalar_value(ty, &s, top),
        }
    }

    /// A value's type with its top-level cv-qualifiers dropped ("to match
    /// GCC").
    fn value_type(&self, ty: QualType) -> QualType {
        self.ast.types.canonical(ty).without_quals()
    }

    fn scalar_value(&mut self, ty: QualType, s: &Scalar<'_>, top: bool) {
        match *s {
            Scalar::Int { signed, bits, .. } => self.int_literal(ty, signed, &bits),
            Scalar::Bool { value, .. } => {
                self.out.push('L');
                self.ty(ty);
                self.out.push_str(if value { "1E" } else { "0E" });
            }
            Scalar::Float { kind, bits, .. } => self.float_literal(ty, kind.bits(), bits),
            Scalar::Null { .. } => self.null_literal(ty),
            Scalar::Pointer { base, offset, designated, .. } => {
                self.pointer_value(ty, base, offset, designated, top);
            }
            Scalar::MemberPtr { member, path, .. } => {
                // `[X] [mc <type>] adL <member> E [E] [E]`.
                if top {
                    self.out.push('X');
                }
                if !path.is_empty() {
                    self.out.push_str("mc");
                    self.ty(ty);
                }
                self.out.push_str("adL");
                self.entity(member);
                self.out.push('E');
                if !path.is_empty() {
                    // The path adjustment (nonzero only for a non-primary
                    // base) is not computed.
                    self.out.push('E');
                }
                if top {
                    self.out.push('E');
                }
            }
            // `L <info-type> <reflection> E` (spec/reflection.md §8.3).
            Scalar::Reflection { id, .. } => self.reflection_literal(ty, Some(id)),
        }
    }

    /// `L <type> <number> E` (`n` for negative; `0`/`1` for `bool`).
    fn int_literal(&mut self, ty: QualType, signed: bool, bits: &kiln_ast::consts::IntBits<'_>) {
        self.out.push('L');
        self.ty(ty);
        let mut v = String::new();
        bits.write_decimal(signed, &mut v);
        if let Some(mag) = v.strip_prefix('-') {
            self.out.push('n');
            self.out.push_str(mag);
        } else {
            self.out.push_str(&v);
        }
        self.out.push('E');
    }

    /// `L <type> <hex> E`: the value bits as fixed-width lowercase hex.
    fn float_literal(&mut self, ty: QualType, width: u32, bits: u128) {
        self.out.push('L');
        self.ty(ty);
        let n = width.div_ceil(4);
        for i in (0..n).rev() {
            let d = (bits >> (4 * i)) & 0xf;
            self.out.push(char::from_digit(d as u32, 16).unwrap_or('0'));
        }
        self.out.push('E');
    }

    /// `L <type> 0 E` (clang `mangleNullPointer`).
    fn null_literal(&mut self, ty: QualType) {
        self.out.push('L');
        self.ty(ty);
        self.out.push_str("0E");
    }

    /// `_Z <encoding>` of `d`, or `_ZTA <value>` for a template parameter
    /// object, sharing this symbol's substitutions.
    fn entity(&mut self, d: DeclId) {
        let ast = self.ast;
        if ast.decls.get(d).kind() == DeclKind::TemplateParamObject {
            self.out.push_str("_ZTA");
            if let Some(v) = ast.templates.param_object_value(d) {
                let ty = ast.decls.get(d).ty().unwrap_or(QualType::INT);
                self.value_in_arg(ty, v, true);
            }
            return;
        }
        self.out.push_str("_Z");
        self.encoding(d, Structor::Complete);
    }

    /// A pointer or reference value (clang's `APValue::LValue` case):
    /// `[X] [ad] L <entity> E [E]` for a whole object, else the subobject
    /// form `[X] [ad] so <referent type> L <entity> E [<offset>]
    /// <union-selector>* [p] E [E]`.
    ///
    /// A `void *` that designates an object of type `designated` mangles
    /// as clang does: the address of that object, converted to the
    /// pointer type (`X cv <type> ad ... E`) when it is the whole
    /// argument.
    fn pointer_value(
        &mut self,
        ty: QualType,
        base: PtrBase,
        offset: u64,
        designated: Option<QualType>,
        top: bool,
    ) {
        let ast = self.ast;
        let is_ref =
            matches!(ast.types.canonical_view(ty), TypeView::LValueRef(_) | TypeView::RValueRef(_));
        let pointee = designated.or_else(|| ast.types.pointee(ty));
        let d = match base {
            PtrBase::Decl(d) | PtrBase::Function(d) => d,
            PtrBase::TypeInfo(t) => {
                self.out.push_str(if top { "X" } else { "" });
                self.out.push_str(if is_ref { "ti" } else { "adti" });
                self.ty(t);
                self.out.push_str(if top { "E" } else { "" });
                return;
            }
            // Temporaries, string literals and frozen objects are not
            // template arguments.
            _ => {
                self.out.push_str("XLi0EE");
                return;
            }
        };
        let path = match base {
            PtrBase::Decl(_) => self.lvalue_path(ast.decls.get(d).ty(), offset, pointee),
            _ => LvaluePath::default(),
        };
        // Inside a class value the member's type is known: no conversion.
        let conv = top && designated.is_some() && !is_ref;
        let wrap = top && (!is_ref || path.nonempty || path.past_end);
        if wrap {
            self.out.push('X');
        }
        if conv {
            self.out.push_str("cv");
            self.ty(ty);
        }
        if !is_ref {
            self.out.push_str("ad");
        }
        if path.nonempty || path.past_end {
            self.out.push_str("so");
            self.ty(pointee.unwrap_or(ty));
            self.out.push('L');
            self.entity(d);
            self.out.push('E');
            if offset != 0 {
                self.out.push_str(&offset.to_string());
            }
            self.out.push_str(&path.selectors);
            if path.past_end {
                self.out.push('p');
            }
            self.out.push('E');
        } else {
            self.out.push('L');
            self.entity(d);
            self.out.push('E');
        }
        if wrap {
            self.out.push('E');
        }
    }

    /// The lvalue path from an object of type `obj` to the subobject at
    /// byte `offset` of type `pointee` (kiln pointers are `(base,
    /// offset)`; clang keeps the path).
    fn lvalue_path(
        &self,
        obj: Option<QualType>,
        offset: u64,
        pointee: Option<QualType>,
    ) -> LvaluePath {
        let ast = self.ast;
        let same = |a: QualType, b: QualType| {
            ast.types.canonical(a).without_quals() == ast.types.canonical(b).without_quals()
        };
        let mut r = LvaluePath::default();
        let Some(mut cur) = obj else {
            r.past_end = offset != 0;
            return r;
        };
        let mut off = offset;
        // Bounded by the nesting depth of the type.
        for _ in 0..4096 {
            if off == 0 && pointee.is_none_or(|p| same(p, cur)) {
                break;
            }
            match ast.shape(cur) {
                Shape::Array { elem, stride, len } if stride != 0 => {
                    let i = off / stride;
                    r.nonempty = true;
                    if i >= len {
                        r.past_end = true;
                        break;
                    }
                    off -= i * stride;
                    cur = elem;
                }
                Shape::Record { members, union } => {
                    let inside = |m: &&Member| {
                        let at = m.offset_bits / 8;
                        m.bit_width.is_none()
                            && m.offset_bits.is_multiple_of(8)
                            && at <= off
                            && (off < at + m.size || (off == at && m.size == 0))
                    };
                    let hit = members
                        .iter()
                        .filter(inside)
                        .find(|m| {
                            off == m.offset_bits / 8 && pointee.is_some_and(|p| same(p, m.ty))
                        })
                        .or_else(|| members.iter().find(inside));
                    let Some(m) = hit else { break };
                    r.nonempty = true;
                    if union {
                        // <union-selector> ::= _ [<number>]
                        let idx = members
                            .iter()
                            .filter(|x| x.field.is_some())
                            .position(|x| x.field == m.field)
                            .unwrap_or(0);
                        r.selectors.push('_');
                        if idx > 0 {
                            r.selectors.push_str(&(idx - 1).to_string());
                        }
                    }
                    off -= m.offset_bits / 8;
                    cur = m.ty;
                }
                _ => {
                    // Past the end of a complete scalar object.
                    r.past_end = !r.nonempty && offset != 0;
                    break;
                }
            }
        }
        r
    }

    /// The pointer-like value relocated at byte `at` of `img`, for a
    /// scalar type.
    fn reloc_at(img: &ByteImage<'_>, at: u64) -> Option<ConstValueId> {
        img.relocs.binary_search_by_key(&at, |r| u64::from(r.at)).ok().map(|i| img.relocs[i].value)
    }

    /// The object of type `ty` at byte `at` of aggregate image `img`
    /// (clang's `Struct`, `Union` and `Array` cases; trailing
    /// zero-initialized elements are dropped).
    fn image_value(&mut self, img: &ByteImage<'_>, ty: QualType, at: u64, top: bool) {
        let ty = self.value_type(ty);
        let shape = self.ast.shape(ty);
        let scalar = !matches!(shape, Shape::Array { .. } | Shape::Record { .. });
        if scalar && let Some(v) = Self::reloc_at(img, at) {
            if let ConstValue::Scalar(s) = self.ast.consts.get(v) {
                self.scalar_value(ty, &s, top);
            }
            return;
        }
        match shape {
            Shape::Bool => {
                self.out.push('L');
                self.ty(ty);
                self.out.push_str(if read_le(img.bytes, at, 1) != 0 { "1E" } else { "0E" });
            }
            Shape::Int { width, signed, .. } => {
                let width = width.clamp(1, kiln_ast::consts::IntBits::MAX_WIDTH);
                let w = read_bits(img.bytes, at * 8, width);
                if let Some(bits) = kiln_ast::consts::IntBits::from_words(width, &w) {
                    self.int_literal(ty, signed, &bits);
                }
            }
            Shape::Float(kind) => {
                let bits = read_le(img.bytes, at, kind.value_bytes());
                self.float_literal(ty, kind.bits(), bits);
            }
            Shape::NullPtr | Shape::Pointer { .. } | Shape::MemberPointer => self.null_literal(ty),
            // A null reflection member (no relocation).
            Shape::Reflection => self.reflection_literal(ty, None),
            Shape::Unknown => {
                self.out.push('L');
                self.ty(ty);
                self.out.push('E');
            }
            Shape::Array { elem, stride, len } => {
                self.out.push_str(if top { "Xtl" } else { "tl" });
                self.ty(ty);
                let mut n = len;
                while n > 0 && self.image_zero(img, elem, at + (n - 1) * stride) {
                    n -= 1;
                }
                for i in 0..n {
                    self.image_value(img, elem, at + i * stride, false);
                }
                self.out.push_str(if top { "EE" } else { "E" });
            }
            Shape::Record { members, union: false } => {
                let unnamed_bf = |m: &Member| m.bit_width.is_some() && m.name.is_empty();
                let mut fields: Vec<&Member> =
                    members.iter().filter(|m| m.field.is_some()).collect();
                while fields.last().is_some_and(|m| unnamed_bf(m) || self.member_zero(img, m, at)) {
                    fields.pop();
                }
                let mut bases: Vec<&Member> =
                    members.iter().filter(|m| m.field.is_none()).collect();
                if fields.is_empty() {
                    while bases.last().is_some_and(|m| self.member_zero(img, m, at)) {
                        bases.pop();
                    }
                }
                self.out.push_str(if top { "Xtl" } else { "tl" });
                self.ty(ty);
                for m in bases {
                    self.image_value(img, m.ty, at + m.offset_bits / 8, false);
                }
                for m in fields {
                    if !unnamed_bf(m) {
                        self.member_value(img, m, at);
                    }
                }
                self.out.push_str(if top { "EE" } else { "E" });
            }
            Shape::Record { members, union: true } => {
                let Some(m) = Self::active_member(img, &members, at) else {
                    self.out.push('L');
                    self.ty(ty);
                    self.out.push('E');
                    return;
                };
                self.out.push_str(if top { "Xtl" } else { "tl" });
                self.ty(ty);
                if !self.union_zero(img, &members, at) {
                    // <braced-expression> ::= di <field source-name> <expr>
                    self.out.push_str("di");
                    if let Some(n) = m.field.and_then(|f| self.union_init_name(f)) {
                        self.source_name(&n);
                    }
                    self.member_value(img, m, at);
                }
                self.out.push_str(if top { "EE" } else { "E" });
            }
        }
    }

    /// Member `m` of the record at byte `at` of `img`.
    fn member_value(&mut self, img: &ByteImage<'_>, m: &Member, at: u64) {
        match m.bit_width {
            Some(width) if width > 0 => {
                let ty = self.value_type(m.ty);
                let w = read_bits(img.bytes, at * 8 + m.offset_bits, width);
                if matches!(self.ast.shape(ty), Shape::Bool) {
                    self.out.push('L');
                    self.ty(ty);
                    self.out.push_str(if w[0] != 0 { "1E" } else { "0E" });
                } else {
                    let signed = matches!(self.ast.shape(ty), Shape::Int { signed: true, .. });
                    if let Some(bits) = kiln_ast::consts::IntBits::from_words(width, &w) {
                        self.int_literal(ty, signed, &bits);
                    }
                }
            }
            Some(_) => {
                self.out.push('L');
                self.ty(m.ty);
                self.out.push_str("0E");
            }
            None => self.image_value(img, m.ty, at + m.offset_bits / 8, false),
        }
    }

    /// The active member of the union at byte `at` of `img`.
    fn active_member<'m>(
        img: &ByteImage<'_>,
        members: &'m [Member],
        at: u64,
    ) -> Option<&'m Member> {
        img.unions
            .iter()
            .filter(|u| u64::from(u.at) == at)
            .find_map(|u| members.iter().find(|m| m.field == Some(u.field)))
    }

    /// The name a union initializer designates (clang `getUnionInitName`):
    /// the field's, or for an anonymous member the first named field of
    /// a depth-first walk.
    fn union_init_name(&self, f: DeclId) -> Option<String> {
        let ast = self.ast;
        let n = decl_name(ast, f);
        if !n.is_empty() {
            return Some(n);
        }
        if ast.decls.field(f).is_some_and(|fd| fd.bit_width.is_some()) {
            return None;
        }
        let ty = ast.decls.get(f).ty()?;
        let TypeView::Record(rd) = ast.types.canonical_view(ty) else { return None };
        let def = ast.definition(rd)?;
        ast.members(def)
            .filter(|&x| ast.decls.get(x).kind() == DeclKind::Field)
            .find_map(|x| self.union_init_name(x))
    }

    /// clang `isZeroInitialized` for the object of type `ty` at byte `at`.
    fn image_zero(&self, img: &ByteImage<'_>, ty: QualType, at: u64) -> bool {
        let shape = self.ast.shape(ty);
        let scalar = !matches!(shape, Shape::Array { .. } | Shape::Record { .. });
        if scalar && let Some(v) = Self::reloc_at(img, at) {
            return matches!(self.ast.consts.get(v), ConstValue::Scalar(Scalar::Null { .. }));
        }
        match shape {
            Shape::Bool => read_le(img.bytes, at, 1) == 0,
            Shape::Int { width, .. } => {
                read_bits(img.bytes, at * 8, width.max(1)).iter().all(|&w| w == 0)
            }
            Shape::Float(kind) => read_le(img.bytes, at, kind.value_bytes()) == 0,
            Shape::NullPtr | Shape::Pointer { .. } | Shape::MemberPointer => true,
            Shape::Unknown | Shape::Reflection => false,
            Shape::Array { elem, stride, len } => {
                (0..len).all(|i| self.image_zero(img, elem, at + i * stride))
            }
            Shape::Record { members, union: false } => members
                .iter()
                .filter(|m| !(m.bit_width.is_some() && m.name.is_empty()))
                .all(|m| self.member_zero(img, m, at)),
            Shape::Record { members, union: true } => self.union_zero(img, &members, at),
        }
    }

    /// Whether member `m` of the record at byte `at` is zero.
    fn member_zero(&self, img: &ByteImage<'_>, m: &Member, at: u64) -> bool {
        match m.bit_width {
            Some(width) => {
                read_bits(img.bytes, at * 8 + m.offset_bits, width.max(1)).iter().all(|&w| w == 0)
            }
            None => self.image_zero(img, m.ty, at + m.offset_bits / 8),
        }
    }

    /// A union is zero-initialized when its first named member is active
    /// and zero, or it has no named member.
    fn union_zero(&self, img: &ByteImage<'_>, members: &[Member], at: u64) -> bool {
        let first = members.iter().find(|m| !(m.bit_width.is_some() && m.name.is_empty()));
        let Some(first) = first else { return true };
        Self::active_member(img, members, at).is_some_and(|a| a.field == first.field)
            && self.member_zero(img, first, at)
    }

    /// A template parameter: its depth appears inside constraints only
    /// (`T [L <depth - 1> _] [<index - 1>] _`, clang
    /// `mangleTemplateParameter`).
    fn template_param_at(&mut self, depth: u32, index: u32) {
        if self.real_depth && !self.head_only && depth > 0 {
            self.out.push_str("TL");
            self.out.push_str(&(depth - 1).to_string());
            self.out.push('_');
            if index > 0 {
                self.out.push_str(&(index - 1).to_string());
            }
            self.out.push('_');
        } else {
            self.template_param(index);
        }
    }

    fn template_param(&mut self, index: u32) {
        if index == 0 {
            self.out.push_str("T_");
        } else {
            self.out.push('T');
            self.out.push_str(&(index - 1).to_string());
            self.out.push('_');
        }
    }

    // ---- types -----------------------------------------------------------

    /// A tag type: its name (a candidate keyed by the declaration).
    fn tag(&mut self, d: DeclId) {
        if self.try_std_abbrev(d) {
            return;
        }
        let key = Sub::Decl(self.ast.first(d));
        if self.try_sub(key) {
            return;
        }
        self.name(d, Structor::Complete);
        self.subs.push(key);
    }

    fn ty(&mut self, q: QualType) {
        let ast = self.ast;
        let q = ast.types.canonical(q);
        let quals = ast.types.effective_quals(q);
        // Arrays carry their element's qualifiers; a vector or complex
        // type is qualified itself (`PKDv4_i`, `PKCd`).
        let is_array = ast.types.element(q).is_some()
            && !matches!(
                ast.types.canonical_view(q),
                TypeView::Vector { .. } | TypeView::DependentVector { .. } | TypeView::Complex(_)
            );
        if !quals.is_empty() && !is_array {
            let key = Sub::Type(q);
            if self.try_sub(key) {
                return;
            }
            if quals.is_restrict() {
                self.out.push('r');
            }
            if quals.is_volatile() {
                self.out.push('V');
            }
            if quals.is_const() {
                self.out.push('K');
            }
            self.ty(q.without_quals());
            self.subs.push(key);
            return;
        }
        let view = ast.types.canonical_view(q);
        match view {
            // A vendor extended type (`u <source-name>`) is substitutable:
            // the SVE types and `std::meta::info` (`u6__info`, §8.3 of
            // spec/reflection.md; GCC's `Dm` is not).
            TypeView::Builtin(b)
                if !b.is_sizeless()
                    && (b != BuiltinType::MetaInfo
                        || self.anon.refl_mode == ReflectionMangling::Gcc) =>
            {
                self.out.push_str(self.builtin_str(b));
                return;
            }
            TypeView::Record(d) | TypeView::Enum(d) => {
                self.tag(d);
                return;
            }
            // An undeduced `auto` / `decltype(auto)` is not a candidate
            // (clang `isTypeSubstitutable`, ABI compat > Clang 6).
            // A constrained placeholder (`Dk`/`DK <type-constraint>`) is
            // not a candidate either.
            TypeView::Auto { keyword, concept: Some(c), args, .. } => {
                self.out.push_str(match keyword {
                    kiln_ast::types::AutoKeyword::DecltypeAuto => "DK",
                    _ => "Dk",
                });
                let args: Vec<ArgView> = args
                    .map(|l| {
                        ast.types.args(l).iter().map(kiln_ast::types::TemplateArg::view).collect()
                    })
                    .unwrap_or_default();
                self.type_constraint(c, &args);
                return;
            }
            TypeView::Auto { keyword, .. } => {
                self.out.push_str(match keyword {
                    kiln_ast::types::AutoKeyword::DecltypeAuto => "Dc",
                    _ => "Da",
                });
                return;
            }
            _ => {}
        }
        let key = Sub::Type(q);
        if self.try_sub(key) {
            return;
        }
        self.compound(q, view);
        self.subs.push(key);
    }

    /// A substitutable type other than a qualified type, builtin or tag.
    fn compound(&mut self, q: QualType, view: TypeView) {
        let ast = self.ast;
        match view {
            TypeView::Pointer(p) => {
                self.out.push('P');
                self.ty(p);
            }
            TypeView::LValueRef(p) => {
                self.out.push('R');
                self.ty(p);
            }
            TypeView::RValueRef(p) => {
                self.out.push('O');
                self.ty(p);
            }
            TypeView::ConstantArray(_) => {
                let n = ast.types.array_size(q).unwrap_or(0);
                self.out.push('A');
                self.out.push_str(&n.to_string());
                self.out.push('_');
                if let Some(e) = ast.types.element(q) {
                    self.ty(e);
                }
            }
            TypeView::IncompleteArray(_) => {
                self.out.push_str("A_");
                if let Some(e) = ast.types.element(q) {
                    self.ty(e);
                }
            }
            // `A <dimension expression> _ <element type>`: a bound that
            // names a template parameter (`T[N]`) keeps the overloads on
            // `const char (&)[N]` and `const wchar_t (&)[N]` apart.
            TypeView::DependentSizedArray { elem, size } => {
                self.dependent_array_bound(size);
                self.out.push('_');
                self.ty(elem);
            }
            TypeView::Function(p) => self.function_type(q, &p, true),
            TypeView::MemberPointer { pointee, class } => self.member_pointer(pointee, class),
            TypeView::Complex(e) => {
                self.out.push('C');
                self.ty(e);
            }
            TypeView::Atomic(e) => {
                self.out.push_str("U7_Atomic");
                self.ty(e);
            }
            TypeView::Vector { elem, count, kind }
                if matches!(kind, VectorKind::Neon | VectorKind::NeonPoly) =>
            {
                let name = self.neon_vector_name(elem, count, kind == VectorKind::NeonPoly);
                self.out.push_str(&name.len().to_string());
                self.out.push_str(&name);
            }
            TypeView::Vector { elem, count, .. } => {
                self.out.push_str("Dv");
                self.out.push_str(&count.to_string());
                self.out.push('_');
                self.ty(elem);
            }
            TypeView::BitInt { signed, width } => {
                self.out.push_str(if signed { "DB" } else { "DU" });
                self.out.push_str(&width.to_string());
                self.out.push('_');
            }
            TypeView::TemplateTypeParm(d) => {
                let (depth, idx) = ast
                    .template_parm(d)
                    .map_or((0, 0), |p| (u32::from(p.depth), u32::from(p.index)));
                self.template_param_at(depth, idx);
            }
            TypeView::CanonTemplateTypeParm { depth, index, .. } => {
                self.template_param_at(depth, index);
            }
            TypeView::PackExpansion { pattern, .. } => {
                self.out.push_str("Dp");
                self.ty(pattern);
            }
            TypeView::Builtin(b) => self.out.push_str(self.builtin_str(b)),
            TypeView::TemplateSpecialization { name, args, .. } => self.dependent_tst(name, args),
            TypeView::DependentName { keyword, nns, name } => {
                self.dependent_name(keyword, nns, name);
            }
            TypeView::DependentTemplateSpecialization { nns, name, args, .. } => {
                self.dependent_template_spec(nns, name, args);
            }
            TypeView::DependentDecltype(p) => self.dependent_decltype(ast.types.profile_expr(p)),
            // A type-dependent GNU `typeof(e)` (clang: a bare vendor name).
            TypeView::TypeOf { .. } => self.out.push_str("u6typeof"),
            // A dependent transform: `u <vendor builtin name> I <base> E`.
            TypeView::UnaryTransform { kind, base, .. } => {
                self.out.push('u');
                self.source_name(kind.spelling());
                self.out.push('I');
                self.ty(base);
                self.out.push('E');
            }
            _ => self.out.push_str("u7unknown"),
        }
    }

    /// <pointer-to-member-type> ::= M <class type> <member type>; a
    /// member function's cv-qualifiers precede its function type.
    fn member_pointer(&mut self, pointee: QualType, class: kiln_ast::ids::TypeId) {
        let ast = self.ast;
        self.out.push('M');
        self.ty(QualType::unqual(class));
        let pointee = ast.types.canonical(pointee);
        if let Some(p) = ast.types.proto(pointee).copied() {
            self.cc_qualifier(&p);
            let mq = p.info().this_quals();
            if mq.is_restrict() {
                self.out.push('r');
            }
            if mq.is_volatile() {
                self.out.push('V');
            }
            if mq.is_const() {
                self.out.push('K');
            }
            let fkey = Sub::Type(pointee);
            if !self.try_sub(fkey) {
                self.function_type(pointee, &p, false);
                self.subs.push(fkey);
            }
        } else {
            self.ty(pointee);
        }
    }

    /// A function type's calling convention as a vendor qualifier (clang
    /// `mangleExtFunctionInfo` / `getCallingConvQualifierName`): only
    /// `ms_abi`, `swiftcall` and `swiftasynccall` are spelled; every other
    /// convention mangles as the default one (WP-808).
    fn cc_qualifier(&mut self, p: &kiln_ast::types::FuncProto) {
        use kiln_ast::types::CallConv as C;
        let q = match p.info().cc() {
            C::Win64 => "ms_abi",
            C::Swift => "swiftcall",
            C::SwiftAsync => "swiftasynccall",
            _ => return,
        };
        self.out.push('U');
        self.source_name(q);
    }

    /// <function-type> ::= [U <cc>] [<CV>] [Do] F [Y] <return> <params>
    /// [<ref>] E (a member pointer writes the convention and the method
    /// cv-qualifiers itself: `cc` is false then).
    fn function_type(&mut self, _q: QualType, p: &kiln_ast::types::FuncProto, cc: bool) {
        let ast = self.ast;
        if cc {
            self.cc_qualifier(p);
        }
        if ast.types.exc_spec(p).is_nothrow() {
            self.out.push_str("Do");
        }
        self.out.push('F');
        let saved = self.fn_depth;
        self.fn_depth = (saved.0 + 1, true);
        self.ty(p.ret());
        self.fn_depth.1 = false;
        let params: Vec<QualType> = ast.types.params(p).to_vec();
        if params.is_empty() && !p.variadic() {
            self.out.push('v');
        }
        for x in params {
            self.ty(self.param_type(x));
        }
        self.fn_depth = saved;
        if p.variadic() {
            self.out.push('z');
        }
        match p.info().ref_qual() {
            RefQual::LValue => self.out.push('R'),
            RefQual::RValue => self.out.push('O'),
            RefQual::None => {}
        }
        self.out.push('E');
    }
}

/// The <builtin-type> code of `b`.
pub fn builtin_code(b: BuiltinType) -> &'static str {
    use BuiltinType as B;
    match b {
        B::Void => "v",
        B::Bool => "b",
        B::Char => "c",
        B::SChar => "a",
        B::UChar => "h",
        B::WChar => "w",
        B::Char8 => "Du",
        B::Char16 => "Ds",
        B::Char32 => "Di",
        B::Short => "s",
        B::UShort => "t",
        B::Int => "i",
        B::UInt => "j",
        B::Long => "l",
        B::ULong => "m",
        B::LongLong => "x",
        B::ULongLong => "y",
        B::Int128 => "n",
        B::UInt128 => "o",
        B::Float16 => "DF16_",
        B::BFloat16 => "DF16b",
        B::Float => "f",
        B::Double => "d",
        B::LongDouble => "e",
        B::Float128 => "g",
        B::Float32 => "DF32_",
        B::Float64 => "DF64_",
        B::StdFloat128 => "DF128_",
        // GCC's `_FloatNx` (Itanium `DF <number> x`).
        B::Float32x => "DF32x",
        B::Float64x => "DF64x",
        B::Half => "Dh",
        B::NullPtr => "Dn",
        B::SvInt8 => "u10__SVInt8_t",
        B::SvInt16 => "u11__SVInt16_t",
        B::SvInt32 => "u11__SVInt32_t",
        B::SvInt64 => "u11__SVInt64_t",
        B::SvUint8 => "u11__SVUint8_t",
        B::SvUint16 => "u12__SVUint16_t",
        B::SvUint32 => "u12__SVUint32_t",
        B::SvUint64 => "u12__SVUint64_t",
        B::SvFloat16 => "u13__SVFloat16_t",
        B::SvFloat32 => "u13__SVFloat32_t",
        B::SvFloat64 => "u13__SVFloat64_t",
        B::SvBfloat16 => "u14__SVBfloat16_t",
        B::SvBool => "u10__SVBool_t",
        B::SvCount => "u11__SVCount_t",
        // The vendor placeholder for `std::meta::info` until WP-102
        // (GCC 16.1 writes `Dm`; spec/reflection.md §8.3).
        B::MetaInfo => "u6__info",
        _ => "u7unknown",
    }
}

fn base36(mut n: usize) -> String {
    const DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    if n == 0 {
        return "0".to_owned();
    }
    let mut v = Vec::new();
    while n > 0 {
        v.push(DIGITS[n % 36]);
        n /= 36;
    }
    v.reverse();
    String::from_utf8(v).unwrap_or_default()
}

/// The Itanium <operator-name> of `op` declared with `arity` operands
/// (the implicit object parameter included): `+ - * &` with one operand
/// are the unary forms.
pub fn operator_code(op: OverloadedOperator, arity: usize) -> &'static str {
    use OverloadedOperator as O;
    let unary = arity == 1;
    match op {
        O::New => "nw",
        O::ArrayNew => "na",
        O::Delete => "dl",
        O::ArrayDelete => "da",
        O::Plus if unary => "ps",
        O::Minus if unary => "ng",
        O::Star if unary => "de",
        O::Amp if unary => "ad",
        O::Plus => "pl",
        O::Minus => "mi",
        O::Star => "ml",
        O::Amp => "an",
        O::Slash => "dv",
        O::Percent => "rm",
        O::Caret => "eo",
        O::Pipe => "or",
        O::Tilde => "co",
        O::Exclaim => "nt",
        O::Equal => "aS",
        O::Less => "lt",
        O::Greater => "gt",
        O::PlusEqual => "pL",
        O::MinusEqual => "mI",
        O::StarEqual => "mL",
        O::SlashEqual => "dV",
        O::PercentEqual => "rM",
        O::CaretEqual => "eO",
        O::AmpEqual => "aN",
        O::PipeEqual => "oR",
        O::LessLess => "ls",
        O::GreaterGreater => "rs",
        O::LessLessEqual => "lS",
        O::GreaterGreaterEqual => "rS",
        O::EqualEqual => "eq",
        O::ExclaimEqual => "ne",
        O::LessEqual => "le",
        O::GreaterEqual => "ge",
        O::Spaceship => "ss",
        O::AmpAmp => "aa",
        O::PipePipe => "oo",
        O::PlusPlus => "pp",
        O::MinusMinus => "mm",
        O::Comma => "cm",
        O::ArrowStar => "pm",
        O::Arrow => "pt",
        O::Call => "cl",
        O::Subscript => "ix",
        O::Coawait => "aw",
    }
}

/// A subobject designation recovered from a byte offset.
#[derive(Default)]
struct LvaluePath {
    /// The path has at least one entry.
    nonempty: bool,
    /// One past the end of an array or complete object (`p`).
    past_end: bool,
    /// `<union-selector>*`.
    selectors: String,
}

/// `width` bits starting at bit `start` of little-endian `bytes`, as
/// normalized words; bits past the end read as zero.
fn read_bits(bytes: &[u8], start: u64, width: u32) -> Vec<u64> {
    let mut w = vec![0u64; kiln_ast::consts::IntBits::words_for(width)];
    for i in 0..u64::from(width) {
        let b = start + i;
        let set = bytes.get((b / 8) as usize).is_some_and(|&x| x >> (b % 8) & 1 != 0);
        if set {
            w[(i / 64) as usize] |= 1 << (i % 64);
        }
    }
    w
}

/// `n` (at most 16) little-endian bytes at `at`.
fn read_le(bytes: &[u8], at: u64, n: u32) -> u128 {
    (0..u64::from(n.min(16))).rev().fold(0u128, |acc, i| {
        (acc << 8) | u128::from(bytes.get((at + i) as usize).copied().unwrap_or(0))
    })
}

/// For the unnamed variable of a namespace-scope anonymous union, the
/// union's first named data member, looking through nested anonymous
/// records (clang `getFirstNamedDataMember`).
fn anon_union_var_field(ast: &Ast, d: DeclId) -> Option<DeclId> {
    if ast.decls.get(d).kind() != DeclKind::Var || !decl_name(ast, d).is_empty() {
        return None;
    }
    first_named_field(ast, ast.types.as_record(ast.decls.get(d).ty()?)?)
}

fn first_named_field(ast: &Ast, r: DeclId) -> Option<DeclId> {
    if !decl_name(ast, r).is_empty() {
        return None;
    }
    ast.members(r).filter(|&m| ast.decls.get(m).kind() == DeclKind::Field).find_map(|f| {
        if decl_name(ast, f).is_empty() {
            first_named_field(ast, ast.types.as_record(ast.decls.get(f).ty()?)?)
        } else {
            Some(f)
        }
    })
}
