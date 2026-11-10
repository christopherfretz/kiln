//! GCC 16.1's spelling of types and declarations for `display_string_of`
//! (spec/reflection.md §3.9).
//!
//! GCC's `display_string_of` formats a type with the diagnostic `%T`
//! directive and a declaration with `%D` (gcc/cp/reflect.cc
//! `eval_display_string_of`), so its strings are those of
//! gcc/cp/error.cc: `dump_type`, `dump_type_prefix`/`dump_type_suffix`,
//! `dump_aggr_type`, `dump_scope`, `dump_decl`, `dump_function_decl` and
//! `dump_template_decl`, over the C++ pretty-printer's padding rules
//! (gcc/c-family/c-pretty-print.cc). kiln reproduces them here, since
//! GCC 16.1 is the reflection reference (§1): `long unsigned int`,
//! `const char*`, `int [3]`, `void(int, ...)`, `ns::Al {aka ns::S}`,
//! `void ns::S::f() const`, `template<class T> struct ns::TT`.
//!
//! Values (template arguments, value and object reflections) go through
//! the constant printer in GCC style (`PrintOptions::gcc`: GCC's floating
//! form, class values, `signed char`, `unsigned char` and the `charN_t`
//! types as integers); a reflection inside a constant prints its operand
//! through `Sema::gcc_reflection_operand` below (WP-1229).

use kiln_ast::decl::{DeclKind, EnumData, FriendKind};
use kiln_ast::names::DeclNameView;
use kiln_ast::types::{ArgView, BuiltinType, ExcSpec, RefQual, TemplateArg, TypeView};
use kiln_ast::{ArgListId, ConstValueId, DeclId, QualType, TemplateNameId};

use crate::Sema;

/// `TFF_SCOPE`: print the scope of a function-local class through the
/// function.
const SCOPE: u32 = 1;
/// `TFF_DECL_SPECIFIERS`.
const DECL_SPECIFIERS: u32 = 1 << 1;
/// `TFF_CLASS_KEY_OR_ENUM`.
const CLASS_KEY: u32 = 1 << 2;
/// `TFF_RETURN_TYPE`.
const RETURN_TYPE: u32 = 1 << 3;
/// `TFF_TEMPLATE_HEADER`.
const TEMPLATE_HEADER: u32 = 1 << 4;
/// `TFF_TEMPLATE_NAME`.
const TEMPLATE_NAME: u32 = 1 << 5;
/// `TFF_UNQUALIFIED_NAME`.
const UNQUALIFIED: u32 = 1 << 6;
/// `TFF_POINTER`: a function type reached through a pointer.
const POINTER: u32 = 1 << 7;
/// `TFF_NO_TEMPLATE_BINDINGS`.
const NO_TEMPLATE_BINDINGS: u32 = 1 << 8;

/// The structural kind of a type once its sugar is looked through, which
/// decides `dump_type_prefix`/`dump_type_suffix` (GCC switches on
/// `TREE_CODE`, which a typedef variant shares with its type).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    Pointer(QualType),
    LRef(QualType),
    RRef(QualType),
    MemData {
        pointee: QualType,
        class: QualType,
    },
    MemFn {
        pointee: QualType,
        class: QualType,
    },
    Array(QualType, Option<u64>),
    /// An array whose bound is value-dependent (`T [size]`).
    DepArray(QualType, kiln_ast::types::ExprOperand),
    Function(QualType),
    Leaf,
}

/// Template parameter levels and their arguments (`None`: bound to
/// themselves), innermost first.
type Levels = Vec<(DeclId, Option<ArgListId>)>;

/// The typedefs a signature names with the types they denote.
type Names = Vec<(QualType, QualType)>;

/// GCC's `cxx_pretty_printer` state: the text and its padding
/// (`pp_before` or `pp_none`).
struct Pp<'s, 'a, 'b> {
    sema: &'s Sema<'a, 'b>,
    out: String,
    pad: bool,
    /// `current_dump_scope`: the context of the function whose declarator
    /// is being printed; a scope equal to it is not repeated
    /// (`void ns::S::g(Inner)`).
    scope: Option<DeclId>,
    /// The template parameters in scope by depth: a canonical parameter
    /// (`type-parameter-0-0` in kiln) prints by its declared name, as GCC's
    /// `TEMPLATE_TYPE_PARM` does (`T (&)[size]`).
    parms: Vec<Vec<DeclId>>,
}

impl Sema<'_, '_> {
    /// `%T` of `t` (gcc/cp/error.cc `type_to_string`): the type, then
    /// ` {aka T'}` with its typedefs stripped when that reads differently.
    pub(crate) fn gcc_type_string(&self, t: QualType) -> String {
        let mut p = Pp::new(self);
        p.dump_type(t, TEMPLATE_HEADER);
        let canon = self.ast.types.canonical(t);
        if canon != t && !self.ast.types.is_dependent(t) {
            let mut aka = Pp::new(self);
            aka.dump_type(canon, TEMPLATE_HEADER);
            if aka.out != p.out {
                p.out.push_str(" {aka ");
                p.out.push_str(&aka.out);
                p.out.push('}');
            }
        }
        p.out
    }

    /// `%D` of declaration `d` (gcc/cp/error.cc `decl_to_string`): a
    /// function with its specifiers and return type, a class with its
    /// key, anything else by its qualified name.
    pub(crate) fn gcc_decl_string(&self, d: DeclId) -> String {
        let k = self.ast.decls.get(d).kind();
        let mut flags = TEMPLATE_HEADER;
        if matches!(
            k,
            DeclKind::Typedef | DeclKind::TypeAlias | DeclKind::CXXRecord | DeclKind::Enum
        ) {
            flags |= CLASS_KEY;
        }
        if k.is_function() {
            flags |= DECL_SPECIFIERS | RETURN_TYPE;
        }
        let mut p = Pp::new(self);
        p.dump_decl(d, flags);
        p.out
    }
}

impl Sema<'_, '_> {
    /// The operand GCC 16.1 prints after `^^` for reflection `r` inside a
    /// constant (`dump_expr` of a `REFLECT_EXPR`: a reflection-valued
    /// template argument or member; spec/reflection.md §3.9), fully
    /// qualified: a type as `%T` without its ` {aka ...}`
    /// (`const ns::S*`), a namespace or alias by its scoped name
    /// (`ns::in::Q`, `::`), a non-member function with its parameter types
    /// (`ns::g(int, double)`), a member function, variable, data member or
    /// enumerator by its scoped name (`ns::S::mf`, `ns::TT<int>::t`), a
    /// template by its scoped name (`ns::TT`), a variable template with its
    /// parameters (`ns::vt<T>`). `None` for the other reflections (values,
    /// objects, base specifiers, parameters, specs, annotations).
    pub(crate) fn gcc_reflection_operand(&self, r: kiln_ast::reflect::Reflectee) -> Option<String> {
        use kiln_ast::reflect::Reflectee;
        let mut p = Pp::new(self);
        match r {
            Reflectee::Type(t) => p.dump_type(t, TEMPLATE_HEADER),
            Reflectee::Namespace(d) | Reflectee::NamespaceAlias(d) => p.dump_decl(d, 0),
            Reflectee::Decl(d) => {
                let ast = &self.ast;
                let k = ast.decls.get(d).kind();
                let member = ast
                    .decls
                    .get(d)
                    .ctx()
                    .is_some_and(|c| ast.decls.get(c).kind() == DeclKind::CXXRecord);
                if k.is_function() && member {
                    p.dump_scope_of(d, 0);
                    p.decl_identifier(d);
                } else {
                    p.dump_decl(d, NO_TEMPLATE_BINDINGS);
                }
            }
            Reflectee::Template(n) => {
                let td = self.ast.types.template_decl(n)?;
                let pattern = self.ast.template_info(td).and_then(|i| i.pattern);
                match pattern {
                    Some(v) if self.ast.decls.get(td).kind() == DeclKind::VarTemplate => {
                        p.dump_decl(v, 0);
                    }
                    _ => {
                        p.dump_scope_of(td, 0);
                        p.decl_identifier(td);
                    }
                }
            }
            _ => return None,
        }
        Some(p.out)
    }
}

impl<'s, 'a, 'b> Pp<'s, 'a, 'b> {
    /// An empty printer over `sema`.
    fn new(sema: &'s Sema<'a, 'b>) -> Self {
        Pp { sema, out: String::new(), pad: false, scope: None, parms: Vec::new() }
    }

    /// The AST, borrowed for `'s` rather than through `self`.
    fn ast(&self) -> &'s kiln_ast::Ast {
        &self.sema.ast
    }

    // ----- the pretty-printer primitives (c-pretty-print.cc) ----------

    /// `pp_c_whitespace`.
    fn whitespace(&mut self) {
        self.out.push(' ');
        self.pad = false;
    }

    /// `pp_c_maybe_whitespace` / `pp_maybe_space`.
    fn maybe_space(&mut self) {
        if self.pad {
            self.whitespace();
        }
    }

    /// `pp_c_ws_string` / `pp_c_identifier`.
    fn ws(&mut self, s: &str) {
        self.maybe_space();
        self.out.push_str(s);
        self.pad = true;
    }

    /// `pp_c_left_paren` and the other punctuators that reset padding.
    fn punct(&mut self, s: &str) {
        self.out.push_str(s);
        self.pad = false;
    }

    /// `pp_cxx_nonconsecutive_character` (`<`, `>`): `A<B<int> >`.
    fn nonconsecutive(&mut self, c: char) {
        if self.out.ends_with(c) {
            self.whitespace();
        }
        self.out.push(c);
        self.pad = false;
    }

    /// `pp_separate_with_comma`.
    fn comma(&mut self) {
        self.punct(", ");
    }

    /// `pp_c_cv_qualifiers`: after `*` or `&` a space first.
    fn cv(&mut self, bits: u8) {
        let q = kiln_ast::types::Quals::from_bits(bits);
        if q.is_empty() {
            return;
        }
        if self.out.ends_with('*') || self.out.ends_with('&') {
            self.whitespace();
        }
        if q.is_const() {
            self.ws("const");
        }
        if q.is_volatile() {
            self.ws("volatile");
        }
        if q.is_restrict() {
            self.ws("__restrict__");
        }
    }

    // ----- types -------------------------------------------------------

    /// The structural kind of `t` with its sugar looked through, carrying
    /// the qualifiers met on the way.
    fn shape(&self, t: QualType) -> (Shape, u8) {
        let types = &self.ast().types;
        let mut cur = t;
        let mut quals = t.quals_bits();
        loop {
            let s = match types.view(cur.ty()) {
                TypeView::Pointer(p) => Shape::Pointer(p),
                TypeView::LValueRef(p) => Shape::LRef(p),
                TypeView::RValueRef(p) => Shape::RRef(p),
                TypeView::MemberPointer { pointee, class } => {
                    let class = QualType::unqual(class);
                    if types.proto(pointee).is_some() {
                        Shape::MemFn { pointee, class }
                    } else {
                        Shape::MemData { pointee, class }
                    }
                }
                TypeView::ConstantArray(a) => Shape::Array(a.elem, Some(a.size)),
                TypeView::IncompleteArray(e) => Shape::Array(e, None),
                TypeView::DependentSizedArray { elem, size } => Shape::DepArray(elem, size),
                TypeView::QualifiedArray { quals: q, base } => {
                    quals |= q.bits();
                    cur = QualType::unqual(base);
                    continue;
                }
                TypeView::Function(_) => Shape::Function(cur),
                _ => match self.desugar_step(cur) {
                    Some(next) => {
                        quals |= next.quals_bits();
                        cur = next;
                        continue;
                    }
                    None => Shape::Leaf,
                },
            };
            // An array's qualifiers belong to its element.
            let s = match s {
                Shape::Array(e, n) if quals != 0 => {
                    let e = QualType::new(e.ty(), e.quals_bits() | quals);
                    quals = 0;
                    Shape::Array(e, n)
                }
                s => s,
            };
            return (s, quals);
        }
    }

    /// One step of sugar removed from `t` (its qualifiers kept), or `None`
    /// for a type with no sugar at the top.
    fn desugar_step(&self, t: QualType) -> Option<QualType> {
        let types = &self.ast().types;
        let inner = match types.view(t.ty()) {
            TypeView::Paren(i) | TypeView::Atomic(i) => i,
            TypeView::Typedef { underlying, .. }
            | TypeView::Using { underlying, .. }
            | TypeView::Decltype { underlying, .. } => underlying,
            TypeView::Elaborated { named, .. } => named,
            TypeView::SubstTemplateTypeParm { replacement, .. } => replacement,
            TypeView::TemplateSpecialization { underlying: Some(u), .. } => u,
            TypeView::InjectedClassName { injected, .. } => injected,
            TypeView::ExtQual { base, .. } => base,
            _ => return None,
        };
        Some(QualType::new(inner.ty(), inner.quals_bits() | t.quals_bits()))
    }

    /// Whether typedef `decl` names an unnamed class or enumeration for
    /// linkage ([dcl.typedef]/9), which GCC then prints by that name as
    /// the class itself.
    fn names_anon_class(&self, decl: DeclId) -> bool {
        let ast = self.ast();
        let Some(u) = ast.decls.typedef(decl).map(|t| ast.types.canonical(t.underlying)) else {
            return false;
        };
        let tag = ast.types.as_record(u).or_else(|| ast.types.as_enum(u));
        tag.and_then(|r| ast.decls.typedef_for_anon(r))
            .is_some_and(|t| ast.first(t) == ast.first(decl)) // same_entity: raw ok
    }

    /// The prototype of function type `t` as written: its parameter and
    /// return types keep their typedefs (`std::size_t`), which GCC prints.
    fn sugared_proto(&self, t: QualType) -> Option<kiln_ast::types::FuncProto> {
        let types = &self.ast().types;
        let mut cur = t;
        loop {
            if let Some(p) = types.proto_of(cur.ty()) {
                return Some(*p);
            }
            cur = self.desugar_step(cur)?;
        }
    }

    /// `dump_type`.
    fn dump_type(&mut self, t: QualType, flags: u32) {
        let types = &self.ast().types;
        match types.view(t.ty()) {
            // A typedef prints by name: `std::size_t`, `const I`.
            TypeView::Typedef { decl, .. } => {
                if self.names_anon_class(decl) {
                    // `typedef struct {} X;`: the class is named `X`.
                    return self.dump_type(self.desugar_step(t).expect("typedef"), flags);
                }
                self.cv(t.quals_bits());
                if flags & UNQUALIFIED == 0 {
                    self.dump_scope_of(decl, flags);
                }
                self.decl_identifier(decl);
                return;
            }
            // An alias template specialization prints by its template-id.
            TypeView::TemplateSpecialization { name, args, alias: true, .. } => {
                self.cv(t.quals_bits());
                if let Some(td) = types.template_decl(name) {
                    if flags & UNQUALIFIED == 0 {
                        self.dump_scope_of(td, flags);
                    }
                    self.decl_identifier(td);
                    let args = types.canonical_args(args);
                    let n = types.args(args).len();
                    self.template_args(args, n, flags & !TEMPLATE_HEADER);
                }
                return;
            }
            _ => {}
        }
        if let Some(next) = self.desugar_step(t) {
            return self.dump_type(next, flags);
        }
        if !matches!(self.shape(t).0, Shape::Leaf) {
            self.dump_type_prefix(t, flags);
            self.dump_type_suffix(t, flags);
            return;
        }
        match types.view(t.ty()) {
            TypeView::Builtin(b) => {
                self.cv(t.quals_bits());
                self.ws(builtin_name(b));
            }
            TypeView::BitInt { signed, width } => {
                self.cv(t.quals_bits());
                if !signed {
                    self.ws("unsigned");
                }
                self.ws("_BitInt(");
                self.out.push_str(&width.to_string());
                self.punct(")");
            }
            TypeView::Record(d) | TypeView::Enum(d) => {
                self.dump_aggr_type(d, t.quals_bits(), flags);
            }
            TypeView::Complex(e) => {
                self.cv(t.quals_bits());
                self.ws("__complex__");
                self.dump_type(e, flags);
            }
            TypeView::Vector { elem, count, .. } => {
                self.cv(t.quals_bits());
                self.ws(&format!("__vector({count})"));
                self.dump_type(elem, flags);
            }
            // A template's parameters, in the templates and members GCC
            // prints through their patterns (`void ns::TT<T, N>::mem(T)`).
            TypeView::TemplateTypeParm(d) => {
                self.cv(t.quals_bits());
                self.decl_identifier(d);
            }
            TypeView::CanonTemplateTypeParm { depth, index, .. }
                if let Some(&d) =
                    self.parms.get(depth as usize).and_then(|l| l.get(index as usize)) =>
            {
                self.cv(t.quals_bits());
                self.decl_identifier(d);
            }
            // `typename T::type` (GCC's TYPENAME_TYPE): `typename`, the
            // qualifying type in full, then the name.
            TypeView::DependentName { nns, name, .. } => {
                self.cv(t.quals_bits());
                self.ws("typename");
                self.nns(nns, flags);
                self.ws(self.ast().symbols().as_str(name));
            }
            TypeView::TemplateSpecialization { name, args, .. } => {
                self.cv(t.quals_bits());
                self.template_name(name, flags);
                let n = types.args(args).len();
                self.template_args(args, n, flags & !TEMPLATE_HEADER);
            }
            _ => {
                // Other dependent forms: kiln's own spelling (no reflection
                // of GCC's holds one).
                self.cv(t.quals_bits());
                let s = self.sema.ty_canonical_str(t.without_quals());
                self.ws(&s);
            }
        }
    }

    /// `dump_type_prefix`.
    fn dump_type_prefix(&mut self, t: QualType, flags: u32) {
        let (shape, quals) = self.shape(t);
        match shape {
            Shape::Pointer(sub) | Shape::LRef(sub) | Shape::RRef(sub) => {
                self.dump_type_prefix(sub, flags);
                if matches!(
                    self.shape(sub).0,
                    Shape::Array(..) | Shape::DepArray(..) | Shape::Function(_)
                ) {
                    self.whitespace();
                    self.punct("(");
                }
                self.out.push_str(match shape {
                    Shape::Pointer(_) => "*",
                    Shape::LRef(_) => "&",
                    _ => "&&",
                });
                self.pad = true;
                self.cv(quals);
            }
            Shape::MemData { pointee, class } => {
                self.dump_type_prefix(pointee, flags);
                self.maybe_space();
                if matches!(self.shape(pointee).0, Shape::Array(..) | Shape::DepArray(..)) {
                    self.punct("(");
                }
                self.dump_type(class, flags);
                self.punct("::");
                self.punct("*");
                self.cv(quals);
                self.pad = true;
            }
            Shape::MemFn { pointee, class } => {
                // `TYPE_PTRMEMFUNC_FN_TYPE`, a pointer to the METHOD_TYPE.
                let ret = self.sugared_proto(pointee).map(|p| p.ret());
                if let Some(ret) = ret {
                    self.dump_type_prefix(ret, flags);
                }
                self.maybe_space();
                self.punct("(");
                if let Some(d) = self.ast().types.as_record(class) {
                    self.dump_aggr_type(d, 0, flags);
                }
                self.punct("::");
                self.punct("*");
                self.cv(quals);
                self.pad = true;
            }
            Shape::Function(f) => {
                if let Some(p) = self.sugared_proto(f) {
                    self.dump_type_prefix(p.ret(), flags);
                }
            }
            Shape::Array(e, _) | Shape::DepArray(e, _) => self.dump_type_prefix(e, flags),
            Shape::Leaf => {
                self.dump_type(t, flags);
                self.pad = true;
            }
        }
    }

    /// `dump_type_suffix`.
    fn dump_type_suffix(&mut self, t: QualType, flags: u32) {
        match self.shape(t).0 {
            Shape::Pointer(sub) | Shape::LRef(sub) | Shape::RRef(sub) => {
                if matches!(
                    self.shape(sub).0,
                    Shape::Array(..) | Shape::DepArray(..) | Shape::Function(_)
                ) {
                    self.punct(")");
                }
                let f = if matches!(self.shape(t).0, Shape::Pointer(_)) {
                    flags | POINTER
                } else {
                    flags
                };
                self.dump_type_suffix(sub, f);
            }
            Shape::MemData { pointee, .. } => {
                if matches!(self.shape(pointee).0, Shape::Array(..) | Shape::DepArray(..)) {
                    self.punct(")");
                }
                self.dump_type_suffix(pointee, flags);
            }
            Shape::MemFn { pointee, .. } => {
                self.punct(")");
                self.function_suffix(pointee, flags | POINTER);
            }
            Shape::Function(f) => self.function_suffix(f, flags),
            Shape::Array(e, n) => {
                self.maybe_space();
                self.punct("[");
                if let Some(n) = n {
                    self.out.push_str(&n.to_string());
                }
                self.punct("]");
                self.dump_type_suffix(e, flags);
            }
            Shape::DepArray(e, size) => {
                self.maybe_space();
                self.punct("[");
                self.dependent_bound(size);
                self.punct("]");
                self.dump_type_suffix(e, flags);
            }
            Shape::Leaf => {}
        }
    }

    /// The `FUNCTION_TYPE` / `METHOD_TYPE` arm of `dump_type_suffix`: the
    /// parameters, the member qualifiers, `noexcept`, then the return
    /// type's suffix.
    fn function_suffix(&mut self, f: QualType, flags: u32) {
        let Some(p) = self.sugared_proto(f) else { return };
        let params = self.ast().types.params(&p).to_vec();
        self.dump_parameters(&params, p.variadic(), flags);
        self.pad = true;
        self.cv(p.info().this_quals().bits());
        self.ref_qualifier(p.info().ref_qual());
        if self.nothrow(&p) {
            self.ws("noexcept");
        }
        self.dump_type_suffix(p.ret(), flags);
    }

    /// Whether prototype `p` is `noexcept` (GCC prints a non-throwing
    /// specification as `noexcept`, `throw()` included).
    fn nothrow(&self, p: &kiln_ast::types::FuncProto) -> bool {
        let e = self.ast().types.exc_spec(p);
        e.is_nothrow() && e != ExcSpec::NothrowAttr
    }

    /// `dump_ref_qualifier`.
    fn ref_qualifier(&mut self, r: RefQual) {
        match r {
            RefQual::None => {}
            RefQual::LValue => {
                self.pad = true;
                self.ws("&");
            }
            RefQual::RValue => {
                self.pad = true;
                self.ws("&&");
            }
        }
    }

    /// A nested-name-specifier of a dependent name, GCC-style: a type in
    /// full (`bsl::enable_if<bsl::is_integral<T>::value, To>::`), a
    /// namespace by its scope.
    fn nns(&mut self, n: kiln_ast::NnsId, flags: u32) {
        use kiln_ast::types::Nns;
        match self.ast().types.nns_view(n) {
            Nns::Global => self.punct("::"),
            Nns::Namespace { ns, .. } | Nns::NamespaceAlias { ns, .. } => {
                self.dump_namespace(ns, flags & SCOPE);
                self.punct("::");
            }
            Nns::Type { ty, .. } => {
                self.dump_type(ty, flags & !CLASS_KEY);
                self.punct("::");
            }
            Nns::Identifier { prefix, name } => {
                if let Some(p) = prefix {
                    self.nns(p, flags);
                }
                self.ws(self.ast().symbols().as_str(name));
                self.punct("::");
            }
            Nns::Splice(_) => self.punct("[: ... :]::"),
        }
    }

    /// The bound of a dependent array: a non-type template parameter by its
    /// name in scope, any other expression as kiln spells it.
    fn dependent_bound(&mut self, size: kiln_ast::types::ExprOperand) {
        let ast = self.ast();
        let e = match size {
            kiln_ast::types::ExprOperand::Expr(e) => Some(e),
            kiln_ast::types::ExprOperand::Profile(p) => ast.types.profile_expr(p),
        };
        let Some(e) = e else { return };
        let inner = self.sema.ignore_paren_imp_casts(e);
        if ast.exprs.get(inner).kind() == kiln_ast::expr::ExprKind::DeclRefExpr {
            let d = ast.exprs.decl(inner);
            if let Some(info) = ast.template_parm(d) {
                let named = self
                    .parms
                    .get(info.depth as usize)
                    .and_then(|l| l.get(info.index as usize))
                    .copied();
                self.decl_identifier(named.unwrap_or(d));
                return;
            }
        }
        let mut s = String::new();
        kiln_ast::types::TypeNames::write_expr(ast, e, &mut s);
        self.ws(&s);
    }

    /// Sets [`Self::parms`] from the template parameter lists enclosing
    /// declaration `d` (itself included, when it is a template pattern).
    fn set_parms_for(&mut self, d: DeclId) {
        let ast = self.ast();
        let mut lists: Vec<(u16, Vec<DeclId>)> = Vec::new();
        let mut cur = Some(d);
        while let Some(c) = cur {
            if let Some(td) = ast.described_template(c)
                && let Some(info) = ast.template_info(td)
            {
                let pl = ast.templates.param_list(info.params);
                lists.push((pl.depth, ast.lists.iter::<DeclId>(pl.params).collect()));
            }
            cur = ast.decls.get(c).ctx();
        }
        let n = lists.iter().map(|l| l.0 as usize + 1).max().unwrap_or(0);
        let mut parms = vec![Vec::new(); n];
        for (depth, l) in lists {
            parms[depth as usize] = l;
        }
        self.parms = parms;
    }

    /// `dump_parameters`.
    fn dump_parameters(&mut self, params: &[QualType], variadic: bool, flags: u32) {
        let flags = flags & !SCOPE;
        self.punct("(");
        let mut first = true;
        for &q in params {
            if !first {
                self.comma();
            }
            first = false;
            self.dump_type(q, flags);
        }
        if variadic {
            if !first {
                self.comma();
            }
            self.ws("...");
        }
        self.punct(")");
    }

    /// `dump_aggr_type` of class or enumeration `d` with qualifiers
    /// `quals`.
    fn dump_aggr_type(&mut self, d: DeclId, quals: u8, flags: u32) {
        let ast = self.ast();
        self.cv(quals);
        if flags & CLASS_KEY != 0 {
            self.ws(self.class_key(d));
        }
        if flags & UNQUALIFIED == 0 {
            self.dump_scope_of(d, flags | SCOPE);
        }
        let flags = flags & !UNQUALIFIED;
        if let Some(l) = ast.closure_lambda(d) {
            // A lambda's "type" is its signature: `<lambda(int)>`.
            self.out.push_str("<lambda");
            if let Some(call) = ast.exprs.lambdas.get(l).call_op
                && let Some(ty) = ast.decls.get(call).ty()
                && let Some(p) = self.sugared_proto(ty)
            {
                let params = ast.types.params(&p).to_vec();
                self.dump_parameters(&params, p.variadic(), flags);
            }
            self.out.push('>');
            return;
        }
        let name = ast.names.simple_spelling(ast.decls.get(d).name(), ast.symbols());
        match name.filter(|s| !s.is_empty()) {
            Some(s) => self.ws(&s),
            None => match ast.decls.typedef_for_anon(d) {
                Some(td) => self.decl_identifier(td),
                None if flags & CLASS_KEY != 0 => self.ws("<unnamed>"),
                None => {
                    let s = format!("<unnamed {}>", self.class_key(d));
                    self.ws(&s);
                }
            },
        }
        // A class template's pattern prints its parameters' names
        // (`ns::TT<T, N>`, the class of a member GCC prints through it).
        if flags & TEMPLATE_NAME == 0
            && let Some(td) = ast.described_template(d)
            && ast.decls.get(td).kind() == DeclKind::ClassTemplate
            && let Some(info) = ast.template_info(td)
        {
            let params: Vec<DeclId> =
                ast.lists.iter::<DeclId>(ast.templates.param_list(info.params).params).collect();
            self.nonconsecutive('<');
            for (i, &p) in params.iter().enumerate() {
                if i > 0 {
                    self.comma();
                }
                self.decl_identifier(p);
            }
            self.nonconsecutive('>');
            return;
        }
        // A class template specialization prints its arguments, without
        // the trailing ones equal to their defaults.
        if ast.decls.get(d).kind() == DeclKind::CXXRecord
            && let Some((_, info)) = ast.spec_info(d)
        {
            let args = ast.types.canonical_args(info.args);
            let all = ast.types.args(args).len();
            let n = match ast.templates.written_args(d) {
                Some(w) => (w as usize).min(all),
                None => ast.types.args(info.print_args).len().min(all),
            };
            if flags & TEMPLATE_NAME == 0 {
                self.template_args(args, n, flags & !TEMPLATE_HEADER);
            }
        }
    }

    /// `class_key_or_enum_as_string`.
    fn class_key(&self, d: DeclId) -> &'static str {
        let ast = self.ast();
        if let Some(e) = ast.decls.enumeration(d) {
            return if e.flags & EnumData::SCOPED != 0 { "enum class" } else { "enum" };
        }
        ast.decls.record(d).map_or("struct", |r| r.tag.keyword())
    }

    /// `dump_template_parms` of a specialization: `<A, B>`.
    fn template_args(&mut self, args: ArgListId, n: usize, flags: u32) {
        let flags = flags & !(CLASS_KEY | TEMPLATE_NAME);
        self.nonconsecutive('<');
        let list: Vec<_> = self.ast().types.args(args).iter().take(n).copied().collect();
        for (i, a) in list.iter().enumerate() {
            let empty_pack = matches!(a.view(), ArgView::Pack(p)
                if self.ast().types.args(p).is_empty());
            if i > 0 && !empty_pack {
                self.comma();
            }
            self.template_arg(a.view(), flags);
        }
        self.nonconsecutive('>');
    }

    /// `dump_template_argument`.
    fn template_arg(&mut self, a: ArgView, flags: u32) {
        match a {
            ArgView::Pack(p) => {
                let list: Vec<_> = self.ast().types.args(p).to_vec();
                let mut first = true;
                for a in list {
                    let empty_pack = matches!(a.view(), ArgView::Pack(p)
                        if self.ast().types.args(p).is_empty());
                    if !first && !empty_pack {
                        self.comma();
                    }
                    first = false;
                    self.template_arg(a.view(), flags);
                }
            }
            ArgView::Type(q) => self.dump_type(q, flags & !CLASS_KEY),
            ArgView::Template(n) | ArgView::TemplateExpansion { name: n, .. } => {
                self.template_name(n, flags);
            }
            ArgView::Value { ty, value } => self.value(ty, value),
            // A dependent argument, in a template GCC prints through its
            // pattern (`ns::TT<T, 5>`).
            ArgView::Expr { expr, .. } => {
                let mut s = String::new();
                kiln_ast::types::TypeNames::write_expr(self.ast(), expr, &mut s);
                self.ws(&s);
            }
            ArgView::Null | ArgView::Profile(_) => self.ws("<expression>"),
        }
    }

    /// A template used as a template argument: its qualified name
    /// (`dump_template_decl` without the header).
    fn template_name(&mut self, n: TemplateNameId, flags: u32) {
        match self.ast().types.template_decl(n) {
            Some(td) => {
                if flags & UNQUALIFIED == 0 {
                    self.dump_scope_of(td, flags);
                }
                self.decl_identifier(td);
            }
            None => self.ws("<template>"),
        }
    }

    /// A constant template argument or value: `pp_c_constant` for the
    /// scalars, kiln's constant printer otherwise.
    fn value(&mut self, ty: QualType, v: ConstValueId) {
        use kiln_ast::consts::Scalar;
        let canon = self.ast().types.canonical(ty);
        if let TypeView::Builtin(b) = self.ast().types.view(canon.ty())
            && matches!(
                b,
                BuiltinType::SChar
                    | BuiltinType::UChar
                    | BuiltinType::WChar
                    | BuiltinType::Char8
                    | BuiltinType::Char16
                    | BuiltinType::Char32
            )
            && let Some(Scalar::Int { signed, bits, .. }) = self.ast().consts.scalar(v)
        {
            let s = if signed {
                bits.to_i128().map(|x| x.to_string())
            } else {
                bits.to_u128().map(|x| x.to_string())
            };
            if let Some(s) = s {
                self.ws(&s);
                return;
            }
        }
        let s = self.sema.value_spelling_gcc(v).unwrap_or_default();
        self.ws(&s);
    }

    // ----- scopes and names --------------------------------------------

    /// `dump_scope (CP_DECL_CONTEXT (d))`: the enclosing namespaces and
    /// classes of `d` followed by `::`. An unscoped enumeration is no
    /// scope of its enumerators; linkage specifications and `export` are
    /// transparent.
    fn dump_scope_of(&mut self, d: DeclId, flags: u32) {
        let ast = self.ast();
        let mut ctx = ast.decls.get(d).ctx();
        while let Some(c) = ctx {
            let n = ast.decls.get(c);
            match n.kind() {
                DeclKind::LinkageSpec | DeclKind::Export => ctx = n.ctx(),
                DeclKind::Enum
                    if ast
                        .decls
                        .enumeration(c)
                        .is_some_and(|e| e.flags & EnumData::SCOPED == 0) =>
                {
                    ctx = n.ctx();
                }
                _ => break,
            }
        }
        let Some(c) = ctx else { return };
        // same_entity: raw ok
        if self.scope == Some(ast.first(c)) {
            return;
        }
        let f = flags & SCOPE;
        let k = ast.decls.get(c).kind();
        match k {
            DeclKind::TranslationUnit => {}
            DeclKind::Namespace => {
                self.dump_namespace(c, f);
                self.punct("::");
            }
            DeclKind::CXXRecord | DeclKind::Enum => {
                let t = self.sema.tag_type(c);
                self.dump_type(t, f);
                self.punct("::");
            }
            _ if k.is_function() && flags & SCOPE != 0 => {
                self.dump_function_decl(c, f | NO_TEMPLATE_BINDINGS);
                self.punct("::");
            }
            _ => {}
        }
    }

    /// `dump_decl` of a `NAMESPACE_DECL` without specifiers.
    fn dump_namespace(&mut self, ns: DeclId, flags: u32) {
        let ast = self.ast();
        if ast.decls.get(ns).kind() == DeclKind::TranslationUnit {
            self.ws("::");
            return;
        }
        if flags & UNQUALIFIED == 0 {
            self.dump_scope_of(ns, flags);
        }
        let name = ast.names.simple_spelling(ast.decls.get(ns).name(), ast.symbols());
        match name.filter(|s| !s.is_empty()) {
            Some(s) => self.ws(&s),
            None => self.ws("{anonymous}"),
        }
    }

    /// The identifier of declaration `d` (`pp_cxx_tree_identifier`).
    fn decl_identifier(&mut self, d: DeclId) {
        let ast = self.ast();
        let s = ast.names.simple_spelling(ast.decls.get(d).name(), ast.symbols());
        self.ws(&s.unwrap_or_default());
    }

    // ----- declarations ------------------------------------------------

    /// `dump_decl`.
    fn dump_decl(&mut self, d: DeclId, flags: u32) {
        let ast = self.ast();
        let k = ast.decls.get(d).kind();
        match k {
            DeclKind::TranslationUnit | DeclKind::Namespace => self.dump_namespace(d, flags),
            DeclKind::NamespaceAlias => {
                if flags & UNQUALIFIED == 0 {
                    self.dump_scope_of(d, flags);
                }
                self.decl_identifier(d);
            }
            DeclKind::CXXRecord | DeclKind::Enum => {
                let t = self.sema.tag_type(d);
                self.dump_type(t, flags);
            }
            DeclKind::Typedef | DeclKind::TypeAlias => {
                if k == DeclKind::TypeAlias && flags & (DECL_SPECIFIERS | CLASS_KEY) != 0 {
                    // `using ns::A = int`.
                    self.ws("using");
                    if flags & UNQUALIFIED == 0 {
                        self.dump_scope_of(d, flags);
                    }
                    self.decl_identifier(d);
                    self.whitespace();
                    self.ws("=");
                    self.whitespace();
                    if let Some(u) = self.ast().decls.typedef(d).map(|t| t.underlying) {
                        self.dump_type(u, flags);
                    }
                    return;
                }
                if flags & DECL_SPECIFIERS != 0 {
                    self.ws("typedef");
                }
                let ty = self.ast().decls.typedef(d).map(|t| t.underlying);
                self.dump_simple_decl(d, ty, flags);
            }
            DeclKind::EnumConstant | DeclKind::Field | DeclKind::IndirectField => {
                let ty = ast.decls.get(d).ty();
                self.dump_simple_decl(d, ty, flags);
            }
            DeclKind::Var | DeclKind::VarTemplateSpecialization | DeclKind::Binding => {
                let ty = ast.decls.get(d).ty();
                self.dump_simple_decl(d, ty, flags);
                // A variable template's pattern: its parameters' names.
                if let Some(td) = ast.described_template(d)
                    && let Some(info) = ast.template_info(td)
                {
                    let params: Vec<DeclId> = ast
                        .lists
                        .iter::<DeclId>(ast.templates.param_list(info.params).params)
                        .collect();
                    self.nonconsecutive('<');
                    for (i, &p) in params.iter().enumerate() {
                        if i > 0 {
                            self.comma();
                        }
                        self.decl_identifier(p);
                    }
                    self.nonconsecutive('>');
                }
                if k == DeclKind::VarTemplateSpecialization
                    && let Some((_, info)) = ast.spec_info(d)
                {
                    let args = ast.types.canonical_args(info.args);
                    let n = ast.types.args(args).len();
                    self.template_args(args, n, flags);
                }
            }
            DeclKind::ClassTemplate
            | DeclKind::FunctionTemplate
            | DeclKind::VarTemplate
            | DeclKind::TypeAliasTemplate
            | DeclKind::Concept => self.dump_template_decl(d, flags),
            _ if k.is_function() => self.dump_function_decl(d, flags),
            _ => {
                let s = self.sema.qualified_decl_name(d);
                self.ws(&s);
            }
        }
    }

    /// `dump_simple_decl` of `d` with type `ty`.
    fn dump_simple_decl(&mut self, d: DeclId, ty: Option<QualType>, flags: u32) {
        let ast = self.ast();
        let k = ast.decls.get(d).kind();
        if flags & DECL_SPECIFIERS != 0 {
            if k == DeclKind::Var && ast.decls.var(d).is_some_and(|v| v.spec.is_constexpr()) {
                self.ws("constexpr");
            }
            if let Some(t) = ty {
                self.dump_type_prefix(t, flags & !UNQUALIFIED);
            }
            self.maybe_space();
        }
        let tparm = matches!(k, DeclKind::NonTypeTemplateParm);
        if flags & UNQUALIFIED == 0 && !tparm {
            self.dump_scope_of(d, flags);
        }
        let flags = flags & !UNQUALIFIED;
        if flags & DECL_SPECIFIERS != 0 && tparm && ast.template_parm(d).is_some_and(|p| p.pack) {
            self.out.push_str("...");
        }
        let name = ast.names.simple_spelling(ast.decls.get(d).name(), ast.symbols());
        match name.filter(|s| !s.is_empty()) {
            Some(s) => self.ws(&s),
            None if k == DeclKind::Binding => self.out.push_str("<structured bindings>"),
            None => self.out.push_str("<anonymous>"),
        }
        if flags & DECL_SPECIFIERS != 0
            && let Some(t) = ty
        {
            self.dump_type_suffix(t, flags);
        }
    }

    /// `dump_function_decl` of function `f`. A function template
    /// specialization or a member of a class template specialization
    /// prints as its pattern followed by the template parameters' bindings
    /// (`T ns::tfn(T) [with T = int]`, `void ns::TT<T, N>::mem(T) [with T
    /// = int; int N = 3]`). The typedefs the signature names
    /// follow (`; size_type = long unsigned int`).
    fn dump_function_decl(&mut self, f: DeclId, flags: u32) {
        let saved = self.scope;
        if flags & NO_TEMPLATE_BINDINGS == 0
            && let Some((pattern, levels)) = self.template_bindings(f)
        {
            self.dump_function_decl_of(pattern, flags);
            // The typedefs the signature names, with what they denote
            // here, unless some level stays a template's own parameters.
            let mut names = Vec::new();
            if levels.iter().all(|l| l.1.is_some()) {
                self.find_typenames(pattern, f, &mut names);
            }
            self.bindings(&levels, &names);
        } else {
            self.dump_function_decl_of(f, flags);
        }
        self.scope = saved;
    }

    /// The pattern of template instantiation `f` with the parameter lists
    /// it binds, innermost first as GCC lists them (`DECL_TEMPLATE_PARMS`
    /// runs from the innermost level out), each with its arguments, or
    /// `None` for a member template's own level, whose parameters stay
    /// bound to themselves; `None` for a function that is not
    /// instantiated from a template.
    fn template_bindings(&self, f: DeclId) -> Option<(DeclId, Levels)> {
        let ast = self.ast();
        let mut levels = Vec::new();
        // The enclosing class template specialization's level.
        let class_level = ast.decls.get(f).ctx().and_then(|c| {
            let (_, info) = ast.spec_info(c)?;
            (ast.decls.get(c).kind() == DeclKind::CXXRecord)
                .then_some((info.template, Some(info.args)))
        });
        if let Some((_, info)) = ast.spec_info(f) {
            let mut tmpl = info.template;
            if let Some((_, m)) = ast.member_spec_info(ast.first(tmpl)) {
                tmpl = m.instantiated_from;
            }
            let pattern = ast.template_info(tmpl)?.pattern?;
            levels.push((tmpl, Some(info.args)));
            levels.extend(class_level);
            return Some((pattern, levels));
        }
        // The pattern of a member template of a class template
        // specialization: printed through the primary's member template.
        if let Some(td) = ast.described_template(f) {
            // The primary's member template: linked from the template decl
            // or from its pattern.
            let general = ast
                .instantiated_from(ast.first(td))
                .filter(|&g| ast.template_info(g).is_some())
                .or_else(|| ast.instantiated_from(f).and_then(|p| ast.described_template(p)));
            if let Some(g) = general
                && g != td // same_entity: raw ok
                && let Some(pattern) = ast.template_info(g).and_then(|t| t.pattern)
            {
                levels.push((g, None));
                levels.push(class_level?);
                return Some((pattern, levels));
            }
        }
        let (_, m) = ast.member_spec_info(ast.first(f))?;
        let pattern = m.instantiated_from;
        // The pattern of a member template of a class template
        // specialization: its own parameters are not bound.
        if let Some(t) = ast.described_template(pattern) {
            levels.push((t, None));
        }
        levels.push(class_level?);
        Some((pattern, levels))
    }

    /// `dump_template_bindings`: ` [with T = int; int N = 3]`, a pack's
    /// arguments in braces; a type parameter bound to itself is skipped,
    /// a non-type one is not (`long unsigned int SIZE = SIZE`).
    fn bindings(&mut self, levels: &Levels, names: &Names) {
        let ast = self.ast();
        let mut first = true;
        for &(tmpl, args) in levels {
            let Some(info) = ast.template_info(tmpl) else { continue };
            let params: Vec<DeclId> =
                ast.lists.iter::<DeclId>(ast.templates.param_list(info.params).params).collect();
            let args: Option<Vec<_>> =
                args.map(|a| ast.types.args(ast.types.canonical_args(a)).to_vec());
            for (i, &p) in params.iter().enumerate() {
                let nontype = ast.decls.get(p).kind() == DeclKind::NonTypeTemplateParm;
                if args.is_none() && !nontype {
                    continue;
                }
                if first {
                    self.whitespace();
                    self.punct("[");
                    self.ws("with");
                    self.whitespace();
                    first = false;
                } else {
                    self.punct("; ");
                }
                if nontype {
                    // GCC's template parameter types carry no typedefs.
                    let ty = ast.decls.get(p).ty().map(|t| ast.types.canonical(t));
                    self.dump_simple_decl(p, ty, DECL_SPECIFIERS);
                } else {
                    self.decl_identifier(p);
                }
                self.whitespace();
                self.out.push('=');
                self.whitespace();
                match &args {
                    None => self.decl_identifier(p),
                    Some(args) => match args.get(i).map(TemplateArg::view) {
                        Some(ArgView::Pack(pk)) => {
                            self.punct("{");
                            self.template_arg(ArgView::Pack(pk), 0);
                            self.punct("}");
                        }
                        Some(a) => self.template_arg(a, 0),
                        None => self.out.push_str("<missing>"),
                    },
                }
            }
        }
        for &(t, v) in names {
            if first {
                self.whitespace();
                self.punct("[");
                self.ws("with");
                self.whitespace();
                first = false;
            } else {
                self.punct("; ");
            }
            self.dump_type(t, 0);
            self.whitespace();
            self.out.push('=');
            self.whitespace();
            self.dump_type(v, 0);
        }
        if !first {
            self.punct("]");
        }
    }

    /// `find_typenames`: the typedefs and dependent names the signature of
    /// `pattern` (return type, then parameters) mentions, in walk order,
    /// each with the canonical type it denotes in instantiation `inst`,
    /// found at the same place of `inst`'s signature.
    fn find_typenames(&self, pattern: DeclId, inst: DeclId, out: &mut Names) {
        let ast = self.ast();
        let (Some(pt), Some(it)) = (ast.decls.get(pattern).ty(), ast.decls.get(inst).ty()) else {
            return;
        };
        let (Some(pp), Some(ip)) = (self.sugared_proto(pt), ast.types.proto(it).copied()) else {
            return;
        };
        self.walk_typenames(pp.ret(), ip.ret(), out);
        let ps = ast.types.params(&pp).to_vec();
        let is = ast.types.params(&ip).to_vec();
        for (p, i) in ps.into_iter().zip(is) {
            self.walk_typenames(p, i, out);
        }
    }

    /// One step of [`Self::find_typenames`]: `pat` as written in the
    /// pattern, `inst` the same place in the instantiation.
    fn walk_typenames(&self, pat: QualType, inst: QualType, out: &mut Names) {
        let types = &self.ast().types;
        let inst = types.canonical(inst);
        let strip = |q: QualType, bits: u8| QualType::new(q.ty(), q.quals_bits() & !bits);
        match types.view(pat.ty()) {
            TypeView::Typedef { decl, .. } => {
                let t = QualType::unqual(pat.ty());
                if !out.iter().any(|&(x, _)| x == t) {
                    out.push((t, strip(inst, pat.quals_bits())));
                }
                // A compound aliased type is walked too (`typedef
                // value_type* pointer` names `value_type`); an alias of an
                // alias is a type of its own with nothing to walk.
                if let Some(u) = self.ast().decls.typedef(decl).map(|d| d.underlying)
                    && matches!(
                        types.view(u.ty()),
                        TypeView::Pointer(_)
                            | TypeView::LValueRef(_)
                            | TypeView::RValueRef(_)
                            | TypeView::ConstantArray(_)
                            | TypeView::Function(_)
                    )
                {
                    self.walk_typenames(u, strip(inst, pat.quals_bits()), out);
                }
            }
            TypeView::DependentName { .. } | TypeView::Decltype { .. } => {
                let t = QualType::unqual(pat.ty());
                if !out.iter().any(|&(x, _)| x == t) {
                    out.push((t, strip(inst, pat.quals_bits())));
                }
            }
            TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => {
                if let Some(i) = types.pointee(inst) {
                    self.walk_typenames(p, i, out);
                }
            }
            TypeView::Paren(i) | TypeView::Elaborated { named: i, .. } => {
                self.walk_typenames(i, inst, out);
            }
            TypeView::ConstantArray(a) => {
                if let TypeView::ConstantArray(b) = types.view(inst.ty()) {
                    self.walk_typenames(a.elem, b.elem, out);
                }
            }
            TypeView::Function(_) => {
                if let (Some(pp), Some(ip)) = (self.sugared_proto(pat), types.proto(inst).copied())
                {
                    self.walk_typenames(pp.ret(), ip.ret(), out);
                    let ps = types.params(&pp).to_vec();
                    let is = types.params(&ip).to_vec();
                    for (p, i) in ps.into_iter().zip(is) {
                        self.walk_typenames(p, i, out);
                    }
                }
            }
            _ => {}
        }
    }

    /// The body of [`Self::dump_function_decl`] for `f` itself.
    fn dump_function_decl_of(&mut self, f: DeclId, flags: u32) {
        let ast = self.ast();
        let saved_parms = std::mem::take(&mut self.parms);
        self.set_parms_for(f);
        let mut show_return = flags & (RETURN_TYPE | DECL_SPECIFIERS) != 0;
        let do_outer_scope = flags & UNQUALIFIED == 0;
        let name_flags = flags & !UNQUALIFIED;
        let flags = name_flags & !TEMPLATE_NAME;
        let k = ast.decls.get(f).kind();
        let ty = ast.decls.get(f).ty();
        let proto = ty.and_then(|t| self.sugared_proto(t));
        let method = matches!(
            k,
            DeclKind::CXXMethod
                | DeclKind::CXXConstructor
                | DeclKind::CXXDestructor
                | DeclKind::CXXConversion
        );
        let first = ast.first(f);
        let spec = self.sema.fn_spec(first);
        if flags & DECL_SPECIFIERS != 0 {
            // `operator new` and `operator delete` members are implicitly
            // static ([class.free]/1).
            let alloc = matches!(
                ast.names.view(ast.decls.get(f).name()),
                DeclNameView::Operator(
                    kiln_ast::names::OverloadedOperator::New
                        | kiln_ast::names::OverloadedOperator::Delete
                        | kiln_ast::names::OverloadedOperator::ArrayNew
                        | kiln_ast::names::OverloadedOperator::ArrayDelete
                )
            );
            if method && (spec.storage() == kiln_ast::decl::FnStorage::Static || alloc) {
                self.ws("static");
            } else if crate::class::is_virtual(ast, f) {
                self.ws("virtual");
            }
            match spec.constexpr_kind() {
                kiln_ast::decl::ConstexprKind::Constexpr => self.ws("constexpr"),
                kiln_ast::decl::ConstexprKind::Consteval => self.ws("consteval"),
                kiln_ast::decl::ConstexprKind::None => {}
            }
        }
        if show_return {
            show_return = !matches!(
                k,
                DeclKind::CXXConversion
                    | DeclKind::CXXConstructor
                    | DeclKind::CXXDestructor
                    | DeclKind::CXXDeductionGuide
            );
        }
        let ret = proto.map(|p| p.ret());
        if show_return && let Some(r) = ret {
            self.dump_type_prefix(r, flags);
        }
        if do_outer_scope {
            let parent = ast.decls.get(f).ctx();
            let in_class = parent.is_some_and(|c| {
                matches!(ast.decls.get(c).kind(), DeclKind::CXXRecord)
                    && ast.decls.get(f).misc().friend() == FriendKind::None
            });
            if in_class {
                let c = parent.expect("checked");
                let t = self.sema.tag_type(c);
                self.dump_type(t, flags);
                self.punct("::");
            } else {
                self.dump_scope_of(f, flags);
            }
        }
        self.dump_function_name(f, name_flags);
        // The rest of the declarator is looked up in the function's scope.
        self.scope = ast.decls.get(f).ctx().map(|c| ast.first(c));
        self.pad = false;
        if let Some(p) = proto {
            // The function type's parameter types: top-level cv-qualifiers
            // dropped ([dcl.fct]/5), their typedefs kept.
            let params: Vec<QualType> =
                ast.types.params(&p).iter().map(|q| q.without_quals()).collect();
            self.dump_parameters(&params, p.variadic(), flags);
            if method {
                self.pad = true;
                self.cv(p.info().this_quals().bits());
                self.ref_qualifier(p.info().ref_qual());
            }
            if show_return && let Some(r) = ret {
                self.dump_type_suffix(r, flags);
            }
        }
        self.parms = saved_parms;
    }

    /// `dump_function_name`.
    fn dump_function_name(&mut self, f: DeclId, flags: u32) {
        let ast = self.ast();
        let name = ast.decls.get(f).name();
        match ast.names.view(name) {
            DeclNameView::Constructor(_) | DeclNameView::Destructor(_) => {
                let class = ast.decls.get(f).ctx();
                if matches!(ast.names.view(name), DeclNameView::Destructor(_)) {
                    self.punct("~");
                }
                match class {
                    Some(c) if ast.closure_lambda(c).is_some() => self.ws("<lambda>"),
                    Some(c) => {
                        let s = ast.names.simple_spelling(ast.decls.get(c).name(), ast.symbols());
                        match s.filter(|s| !s.is_empty()) {
                            Some(s) => self.ws(&s),
                            None => self.ws("<constructor>"),
                        }
                    }
                    None => {}
                }
            }
            DeclNameView::Conversion(t) => {
                self.ws("operator");
                self.dump_type(t, flags);
            }
            DeclNameView::Operator(op) => {
                let s = op.spelling();
                let s = match s.as_bytes().first() {
                    Some(c) if c.is_ascii_alphabetic() => {
                        format!("operator {}", s.replace("[]", " []"))
                    }
                    _ => format!("operator{s}"),
                };
                self.ws(&s);
            }
            DeclNameView::LiteralOperator(sym) => {
                let s = format!("operator\"\"{}", ast.symbols().as_str(sym));
                self.ws(&s);
            }
            _ => self.decl_identifier(f),
        }
        // A function template specialization: its arguments.
        if flags & TEMPLATE_NAME == 0
            && let Some((_, info)) = ast.spec_info(f)
        {
            let args = ast.types.canonical_args(info.args);
            let n = ast.types.args(args).len();
            self.template_args(args, n, flags);
        }
    }

    /// `dump_template_decl` with the template header: `template<class
    /// T> struct ns::TT`, `template<class T> void ns::f(T)`.
    fn dump_template_decl(&mut self, td: DeclId, flags: u32) {
        let ast = self.ast();
        let Some(info) = ast.template_info(td).copied() else {
            let s = self.sema.qualified_decl_name(td);
            self.ws(&s);
            return;
        };
        let mut flags = flags;
        if flags & TEMPLATE_HEADER != 0 {
            self.template_header(info.params);
            flags |= DECL_SPECIFIERS;
        }
        let k = ast.decls.get(td).kind();
        match (k, info.pattern) {
            (DeclKind::ClassTemplate, Some(p)) => {
                let mut f = (flags & !CLASS_KEY) | TEMPLATE_NAME;
                if flags & DECL_SPECIFIERS != 0 {
                    f |= CLASS_KEY;
                }
                self.dump_aggr_type(p, 0, f);
            }
            (DeclKind::FunctionTemplate, Some(p)) => {
                self.dump_function_decl(p, flags | TEMPLATE_NAME);
            }
            (DeclKind::Concept, _) => {
                if flags & DECL_SPECIFIERS != 0 {
                    self.ws("concept");
                }
                if flags & UNQUALIFIED == 0 {
                    self.dump_scope_of(td, flags);
                }
                self.decl_identifier(td);
            }
            (_, Some(p)) => self.dump_decl(p, flags | TEMPLATE_NAME),
            (_, None) => self.decl_identifier(td),
        }
    }

    /// `template<class T, int N> ` before a template's declaration.
    fn template_header(&mut self, params: kiln_ast::TemplateParamListId) {
        let ast = self.ast();
        let list: Vec<DeclId> =
            ast.lists.iter::<DeclId>(ast.templates.param_list(params).params).collect();
        self.ws("template");
        self.nonconsecutive('<');
        for (i, &p) in list.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            self.template_parameter(p);
        }
        self.nonconsecutive('>');
        self.whitespace();
    }

    /// `dump_template_parameter` with specifiers: `class T`, `class ...
    /// Ts`, `int N`, `template<class> class TT`.
    fn template_parameter(&mut self, p: DeclId) {
        let ast = self.ast();
        let info = ast.template_parm(p).copied();
        match ast.decls.get(p).kind() {
            DeclKind::TemplateTypeParm => {
                self.ws("class");
                if info.is_some_and(|i| i.pack) {
                    self.ws("...");
                }
                let s = ast.names.simple_spelling(ast.decls.get(p).name(), ast.symbols());
                if let Some(s) = s.filter(|s| !s.is_empty()) {
                    self.ws(&s);
                }
            }
            DeclKind::TemplateTemplateParm => {
                if let Some(l) = info.and_then(|i| i.params) {
                    self.template_header(l);
                }
                self.ws("class");
                if info.is_some_and(|i| i.pack) {
                    self.ws("...");
                }
                let s = ast.names.simple_spelling(ast.decls.get(p).name(), ast.symbols());
                if let Some(s) = s.filter(|s| !s.is_empty()) {
                    self.ws(&s);
                }
            }
            _ => {
                let ty = ast.decls.get(p).ty().map(|t| ast.types.canonical(t));
                self.dump_simple_decl(p, ty, DECL_SPECIFIERS);
            }
        }
    }
}

/// GCC's names of the fundamental types (gcc/c-family/c-common.cc
/// `c_common_nodes_and_builtins`): `long unsigned int`, `short int`,
/// `__int128 unsigned`.
fn builtin_name(b: BuiltinType) -> &'static str {
    use BuiltinType as B;
    match b {
        B::Short => "short int",
        B::UShort => "short unsigned int",
        B::Long => "long int",
        B::ULong => "long unsigned int",
        B::LongLong => "long long int",
        B::ULongLong => "long long unsigned int",
        B::UInt128 => "__int128 unsigned",
        b => b.name(),
    }
}
