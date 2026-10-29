//! A tiny class-definition model shared by the `kiln_abi` tests: each
//! [`Model`] renders both as C++ source (for clang) and as a kiln AST
//! built with `AstBuilder` (for `kiln_abi`), so the two layouts can be
//! compared. Items are declared in order; a type may only name earlier
//! items.
#![allow(dead_code, reason = "each test binary uses a subset")]

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::io::Write as _;
use std::process::{Command, Stdio};
use std::sync::Arc;

use kiln_abi::target::Target;
use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::{
    Access, BaseSpec, ConstexprKind, DeclFlags, DeclKind, DefinitionBits, FieldData, FnSpec,
    TagKind,
};
use kiln_ast::expr::Radix;
use kiln_ast::names::DeclName;
use kiln_ast::types::{BuiltinType, ExcSpec, FuncInfo, Quals, TypeContext, VectorKind};
use kiln_ast::{Ast, AttrId, DeclHead, DeclId, ExprId, QualType, RawId, TypeId};
use kiln_source::Loc;
use kiln_source::symbol::Interner;

const L: Loc = Loc::INVALID;

/// A type in the model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ty {
    /// A builtin type.
    B(BuiltinType),
    /// `[unsigned] _BitInt(N)`.
    BitInt(bool, u32),
    /// `T *`.
    Ptr(Box<Ty>),
    /// `T &`.
    LRef(Box<Ty>),
    /// `T &&`.
    RRef(Box<Ty>),
    /// `T[N]`.
    Arr(Box<Ty>, u64),
    /// `T[]` (a flexible array member).
    Flex(Box<Ty>),
    /// The record, enum or typedef item with this index.
    Named(usize),
    /// `_Complex T`.
    Complex(Box<Ty>),
    /// `_Atomic(T)`.
    Atomic(Box<Ty>),
    /// `const` (1) / `volatile` (2) `T` (not on arrays or functions).
    Cv(Box<Ty>, u8),
    /// `T C::*` with `C` a record item.
    MemPtr(Box<Ty>, usize),
    /// `R (P...)` (only under a pointer).
    Func(Box<Ty>, Vec<Ty>),
    /// `decltype(nullptr)`.
    NullptrT,
}

impl Ty {
    pub fn b(b: BuiltinType) -> Ty {
        Ty::B(b)
    }
    pub fn ptr(self) -> Ty {
        Ty::Ptr(Box::new(self))
    }
    pub fn arr(self, n: u64) -> Ty {
        Ty::Arr(Box::new(self), n)
    }
}

/// One non-static data member.
#[derive(Clone, Debug, Default)]
pub struct FieldDef {
    pub name: Option<String>,
    pub ty: Option<Ty>,
    pub bits: Option<u64>,
    /// `alignas(N)`.
    pub alignas: Option<u64>,
    /// `__attribute__((aligned(N)))`.
    pub aligned: Option<u64>,
    pub packed: bool,
    pub nua: bool,
    pub private: bool,
}

impl FieldDef {
    pub fn new(name: &str, ty: Ty) -> FieldDef {
        FieldDef { name: Some(name.to_owned()), ty: Some(ty), ..FieldDef::default() }
    }
    pub fn bits(name: Option<&str>, ty: Ty, w: u64) -> FieldDef {
        FieldDef {
            name: name.map(str::to_owned),
            ty: Some(ty),
            bits: Some(w),
            ..FieldDef::default()
        }
    }
    fn ty(&self) -> &Ty {
        self.ty.as_ref().expect("field type")
    }
}

/// How a member function is defined.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MethodKind {
    /// Declared; defined out of line when the record is.
    Normal,
    /// `consteval`, defined in the class.
    Consteval,
    /// `= delete`.
    Deleted,
}

/// A member function.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools, reason = "mirrors the C++ declaration's specifiers")]
pub struct MethodDef {
    pub name: String,
    /// The return type (`Ty::B(Void)` for `void`).
    pub ret: Ty,
    pub params: Vec<Ty>,
    pub is_const: bool,
    /// `virtual` written.
    pub virtual_written: bool,
    /// `= 0`.
    pub pure: bool,
    pub kind: MethodKind,
    /// Virtual, written or by overriding: set by [`Model::push`].
    pub is_virtual: bool,
}

impl MethodDef {
    /// Same name, parameter types and cv-qualifier: `self` overrides
    /// `other` if `other` is a virtual function of a base
    /// ([class.virtual]/2).
    pub fn same_signature(&self, other: &MethodDef) -> bool {
        self.name == other.name && self.params == other.params && self.is_const == other.is_const
    }
}

/// A user-declared destructor.
#[derive(Clone, Copy, Debug)]
pub struct DtorDef {
    pub virtual_written: bool,
    pub pure: bool,
    /// Declared before `methods[pos]` (or last).
    pub pos: usize,
}

/// A class definition.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools, reason = "mirrors the C++ class's attributes")]
pub struct RecDef {
    pub name: String,
    pub tag: TagKind,
    pub fields: Vec<FieldDef>,
    /// `__attribute__((packed))`.
    pub packed: bool,
    /// `__attribute__((aligned(N)))` (`Some(0)`: no argument).
    pub aligned: Option<u64>,
    /// `alignas(N)`.
    pub alignas: Option<u64>,
    /// A user-provided default constructor `S() {}` (not POD).
    pub user_ctor: bool,
    /// Public base classes: model item indices, in order.
    pub bases: Vec<usize>,
    /// The members of `bases` that are virtual base classes.
    pub virtual_bases: Vec<usize>,
    /// `#pragma pack(push, N)` in effect at the definition (bytes).
    pub pack: Option<u16>,
    /// Member functions, in declaration order.
    pub methods: Vec<MethodDef>,
    pub dtor: Option<DtorDef>,
    /// The user constructor and the member functions are defined out of
    /// line, so clang emits them (and with them the vtable and VTT).
    pub out_of_line: bool,
    /// Set by [`Model::push`]: the destructor (declared or implicit) is
    /// virtual.
    pub dtor_virtual: bool,
    /// Set by [`Model::push`]: declares or inherits a virtual function.
    pub polymorphic: bool,
    /// Set by [`Model::push`]: has a virtual base class.
    pub has_vbases: bool,
}

impl RecDef {
    pub fn new(name: &str, tag: TagKind, fields: Vec<FieldDef>) -> RecDef {
        RecDef {
            name: name.to_owned(),
            tag,
            fields,
            packed: false,
            aligned: None,
            alignas: None,
            user_ctor: false,
            bases: Vec::new(),
            virtual_bases: Vec::new(),
            pack: None,
            methods: Vec::new(),
            dtor: None,
            out_of_line: false,
            dtor_virtual: false,
            polymorphic: false,
            has_vbases: false,
        }
    }

    pub fn is_virtual_base(&self, b: usize) -> bool {
        self.virtual_bases.contains(&b)
    }

    /// Polymorphic or with a virtual base (clang `isDynamicClass`).
    pub fn dynamic(&self) -> bool {
        self.polymorphic || self.has_vbases
    }
}

/// The key of a destructor in [`Model::overridden`] results.
pub const DTOR: usize = usize::MAX;

/// An enumeration with a fixed underlying type (`None`: unfixed, `int`).
#[derive(Clone, Debug)]
pub struct EnumDef {
    pub name: String,
    pub fixed: Option<BuiltinType>,
}

/// `typedef T Name [__attribute__((aligned(N)))]` or a vector typedef.
#[derive(Clone, Debug)]
pub struct TypedefDef {
    pub name: String,
    pub ty: Ty,
    pub aligned: Option<u64>,
    /// `__attribute__((vector_size(N)))` bytes: a generic vector of `ty`.
    pub vector_bytes: Option<u64>,
}

/// One declaration.
#[derive(Clone, Debug)]
pub enum Item {
    Record(RecDef),
    Enum(EnumDef),
    Typedef(TypedefDef),
}

/// A sequence of declarations.
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub items: Vec<Item>,
}

fn builtin_spelling(b: BuiltinType) -> &'static str {
    b.name()
}

impl Model {
    /// Appends `item`; a record's derived facts (virtuality,
    /// polymorphism) are computed here.
    pub fn push(&mut self, mut item: Item) -> usize {
        if let Item::Record(r) = &mut item {
            self.finish(r);
        }
        self.items.push(item);
        self.items.len() - 1
    }

    /// The record item `i`.
    pub fn rec(&self, i: usize) -> &RecDef {
        match &self.items[i] {
            Item::Record(r) => r,
            _ => panic!("item {i} is not a record"),
        }
    }

    /// Computes the facts [`Model::push`] sets.
    pub fn finish(&self, r: &mut RecDef) {
        for k in 0..r.methods.len() {
            let m = &r.methods[k];
            let v = m.virtual_written || m.pure || !self.overridden(&r.bases, m).is_empty();
            r.methods[k].is_virtual = v;
        }
        r.dtor_virtual = r.dtor.is_some_and(|d| d.virtual_written || d.pure)
            || !self.overridden_dtors(&r.bases).is_empty();
        r.polymorphic = r.dtor_virtual
            || r.methods.iter().any(|m| m.is_virtual)
            || r.bases.iter().any(|&b| self.rec(b).polymorphic);
        r.has_vbases =
            !r.virtual_bases.is_empty() || r.bases.iter().any(|&b| self.rec(b).has_vbases);
    }

    /// The virtual functions a member `m` of a class with bases `bases`
    /// directly overrides, as `(record, method index)` (clang
    /// `Sema::AddOverriddenMethods`: depth-first over the bases, each
    /// path stopping at the first class declaring a virtual function
    /// with `m`'s signature).
    pub fn overridden(&self, bases: &[usize], m: &MethodDef) -> Vec<(usize, usize)> {
        self.search_bases(bases, &|r: &RecDef| {
            r.methods.iter().position(|o| o.is_virtual && o.same_signature(m))
        })
    }

    /// The destructors a destructor of a class with bases `bases`
    /// directly overrides, as `(record, DTOR)`.
    pub fn overridden_dtors(&self, bases: &[usize]) -> Vec<(usize, usize)> {
        self.search_bases(bases, &|r: &RecDef| r.dtor_virtual.then_some(DTOR))
    }

    fn search_bases(
        &self,
        bases: &[usize],
        find: &dyn Fn(&RecDef) -> Option<usize>,
    ) -> Vec<(usize, usize)> {
        fn visit(
            model: &Model,
            c: usize,
            find: &dyn Fn(&RecDef) -> Option<usize>,
            seen: &mut HashSet<usize>,
            out: &mut Vec<(usize, usize)>,
        ) {
            if !seen.insert(c) {
                // Every path through `c` finds the same functions.
                return;
            }
            let r = model.rec(c);
            if let Some(k) = find(r) {
                if !out.contains(&(c, k)) {
                    out.push((c, k));
                }
                return;
            }
            for &b in &r.bases {
                visit(model, b, find, seen, out);
            }
        }
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        for &b in bases {
            visit(self, b, find, &mut seen, &mut out);
        }
        out
    }

    /// The subobject graph of record `i`: `(class, children)` per node,
    /// node 0 the complete object, each virtual base shared; `None` past
    /// `cap` nodes.
    pub fn subobject_graph(&self, i: usize, cap: usize) -> Option<Vec<(usize, Vec<usize>)>> {
        self.subobject_graph_of(i, self.rec(i), cap)
    }

    /// [`Model::subobject_graph`] of `r` (record `root`, possibly not
    /// pushed yet).
    pub fn subobject_graph_of(
        &self,
        root: usize,
        r: &RecDef,
        cap: usize,
    ) -> Option<Vec<(usize, Vec<usize>)>> {
        fn build(
            model: &Model,
            c: usize,
            cap: usize,
            nodes: &mut Vec<(usize, Vec<usize>)>,
            vmap: &mut HashMap<usize, usize>,
            r: &RecDef,
        ) -> Option<usize> {
            let id = nodes.len();
            nodes.push((c, Vec::new()));
            if nodes.len() > cap {
                return None;
            }
            for &b in &r.bases {
                let child = if r.is_virtual_base(b) {
                    if let Some(&n) = vmap.get(&b) {
                        n
                    } else {
                        let n = build(model, b, cap, nodes, vmap, model.rec(b))?;
                        vmap.insert(b, n);
                        n
                    }
                } else {
                    build(model, b, cap, nodes, vmap, model.rec(b))?
                };
                nodes[id].1.push(child);
            }
            Some(id)
        }
        let mut nodes = Vec::new();
        build(self, root, cap, &mut nodes, &mut HashMap::new(), r)?;
        Some(nodes)
    }

    pub fn name_of(&self, i: usize) -> &str {
        match &self.items[i] {
            Item::Record(r) => &r.name,
            Item::Enum(e) => &e.name,
            Item::Typedef(t) => &t.name,
        }
    }

    /// The record items, by index.
    pub fn records(&self) -> impl Iterator<Item = (usize, &RecDef)> {
        self.items.iter().enumerate().filter_map(|(i, it)| match it {
            Item::Record(r) => Some((i, r)),
            _ => None,
        })
    }

    // ------------------------------------------------------------------
    // C++ source
    // ------------------------------------------------------------------

    fn is_array_like(t: &Ty) -> bool {
        matches!(t, Ty::Arr(..) | Ty::Flex(_) | Ty::Func(..))
    }

    /// `ty` declaring `inner` (clang declarator syntax).
    pub fn declarator(&self, ty: &Ty, inner: &str) -> String {
        let join = |base: &str, inner: &str| {
            if inner.is_empty() { base.to_owned() } else { format!("{base} {inner}") }
        };
        match ty {
            Ty::B(b) => join(builtin_spelling(*b), inner),
            Ty::BitInt(signed, w) => {
                join(&format!("{}_BitInt({w})", if *signed { "" } else { "unsigned " }), inner)
            }
            Ty::Named(i) => join(self.name_of(*i), inner),
            Ty::NullptrT => join("decltype(nullptr)", inner),
            Ty::Complex(t) => format!("_Complex {}", self.declarator(t, inner)),
            Ty::Atomic(t) => join(&format!("_Atomic({})", self.declarator(t, "")), inner),
            Ty::Ptr(t) | Ty::LRef(t) | Ty::RRef(t) | Ty::MemPtr(t, _) => {
                let sigil = match ty {
                    Ty::Ptr(_) => "*".to_owned(),
                    Ty::LRef(_) => "&".to_owned(),
                    Ty::RRef(_) => "&&".to_owned(),
                    Ty::MemPtr(_, c) => format!("{}::*", self.name_of(*c)),
                    _ => unreachable!(),
                };
                let s = format!("{sigil}{inner}");
                let s = if Self::is_array_like(t) { format!("({s})") } else { s };
                self.declarator(t, &s)
            }
            Ty::Cv(t, q) => {
                let cv = match q {
                    1 => "const",
                    2 => "volatile",
                    _ => "const volatile",
                };
                match &**t {
                    Ty::Ptr(_) | Ty::MemPtr(..) => {
                        // `T * const x`: qualify the pointer declarator.
                        let (Ty::Ptr(p) | Ty::MemPtr(p, _)) = &**t else { unreachable!() };
                        let sigil = match &**t {
                            Ty::MemPtr(_, c) => format!("{}::*", self.name_of(*c)),
                            _ => "*".to_owned(),
                        };
                        let s = format!("{sigil} {cv} {inner}").trim_end().to_owned();
                        let s = if Self::is_array_like(p) { format!("({s})") } else { s };
                        self.declarator(p, &s)
                    }
                    _ => format!("{cv} {}", self.declarator(t, inner)),
                }
            }
            Ty::Arr(t, n) => self.declarator(t, &format!("{inner}[{n}]")),
            Ty::Flex(t) => self.declarator(t, &format!("{inner}[]")),
            Ty::Func(r, ps) => {
                let params: Vec<String> = ps.iter().map(|p| self.declarator(p, "")).collect();
                self.declarator(r, &format!("{inner}({})", params.join(", ")))
            }
        }
    }

    /// The model as C++ source; `sizeof` of every record is taken so
    /// clang lays (and dumps) each one.
    pub fn cpp(&self) -> String {
        let mut s = String::new();
        for item in &self.items {
            match item {
                Item::Enum(e) => match e.fixed {
                    Some(b) => {
                        let _ = writeln!(s, "enum {} : {} {{}};", e.name, builtin_spelling(b));
                    }
                    None => {
                        let _ = writeln!(s, "enum {} {{ {}_0 }};", e.name, e.name);
                    }
                },
                Item::Typedef(t) => {
                    let mut attrs = Vec::new();
                    if let Some(n) = t.vector_bytes {
                        attrs.push(format!("vector_size({n})"));
                    }
                    if let Some(n) = t.aligned {
                        attrs.push(format!("aligned({n})"));
                    }
                    let attr = if attrs.is_empty() {
                        String::new()
                    } else {
                        format!(" __attribute__(({}))", attrs.join(", "))
                    };
                    let _ = writeln!(s, "typedef {}{attr};", self.declarator(&t.ty, &t.name));
                }
                Item::Record(r) => self.record_cpp(r, &mut s),
            }
        }
        for (_, r) in self.records() {
            let _ = writeln!(s, "static_assert(sizeof({}) >= 0, \"\");", r.name);
        }
        s
    }

    #[allow(clippy::too_many_lines, reason = "one linear rendering of a class definition")]
    fn record_cpp(&self, r: &RecDef, s: &mut String) {
        let kw = match r.tag {
            TagKind::Struct => "struct",
            TagKind::Class => "class",
            TagKind::Union => "union",
        };
        let mut gnu = Vec::new();
        if r.packed {
            gnu.push("packed".to_owned());
        }
        match r.aligned {
            Some(0) => gnu.push("aligned".to_owned()),
            Some(n) => gnu.push(format!("aligned({n})")),
            None => {}
        }
        let mut head = kw.to_owned();
        if !gnu.is_empty() {
            let _ = write!(head, " __attribute__(({}))", gnu.join(", "));
        }
        if let Some(n) = r.alignas {
            let _ = write!(head, " alignas({n})");
        }
        if let Some(n) = r.pack {
            let _ = writeln!(s, "#pragma pack(push, {n})");
        }
        let bases: Vec<String> = r
            .bases
            .iter()
            .map(|&i| {
                let v = if r.is_virtual_base(i) { "virtual " } else { "" };
                format!("public {v}{}", self.name_of(i))
            })
            .collect();
        let base_clause =
            if bases.is_empty() { String::new() } else { format!(" : {}", bases.join(", ")) };
        let _ = writeln!(s, "{head} {}{base_clause} {{", r.name);
        for f in &r.fields {
            let access = if f.private { "private" } else { "public" };
            let mut line = format!("{access}: ");
            if f.nua {
                line.push_str("[[no_unique_address]] ");
            }
            if let Some(n) = f.alignas {
                let _ = write!(line, "alignas({n}) ");
            }
            line.push_str(&self.declarator(f.ty(), f.name.as_deref().unwrap_or("")));
            if let Some(w) = f.bits {
                let _ = write!(line, " : {w}");
            }
            let mut fa = Vec::new();
            if f.packed {
                fa.push("packed".to_owned());
            }
            if let Some(n) = f.aligned {
                fa.push(format!("aligned({n})"));
            }
            if !fa.is_empty() {
                let _ = write!(line, " __attribute__(({}))", fa.join(", "));
            }
            let _ = writeln!(s, "  {line};");
        }
        if r.user_ctor {
            if r.out_of_line {
                let _ = writeln!(s, "public: {}();", r.name);
            } else {
                let _ = writeln!(s, "public: {}() {{}}", r.name);
            }
        }
        for k in 0..=r.methods.len() {
            if let Some(d) = r.dtor.filter(|d| d.pos == k) {
                let v = if d.virtual_written { "virtual " } else { "" };
                let p = if d.pure { " = 0" } else { "" };
                let _ = writeln!(s, "public: {v}~{}(){p};", r.name);
            }
            let Some(m) = r.methods.get(k) else { break };
            let mut line = String::from("public: ");
            if m.kind == MethodKind::Consteval {
                line.push_str("consteval ");
            }
            if m.virtual_written {
                line.push_str("virtual ");
            }
            line.push_str(&self.declarator(&m.ret, &self.method_head(m, "")));
            if m.pure {
                line.push_str(" = 0");
            }
            match m.kind {
                MethodKind::Normal => line.push(';'),
                MethodKind::Deleted => line.push_str(" = delete;"),
                MethodKind::Consteval if m.ret == Ty::B(BuiltinType::Void) => line.push_str(" {}"),
                MethodKind::Consteval => line.push_str(" { return {}; }"),
            }
            let _ = writeln!(s, "{line}");
        }
        let _ = writeln!(s, "}};");
        if r.pack.is_some() {
            let _ = writeln!(s, "#pragma pack(pop)");
        }
        if r.out_of_line {
            if r.user_ctor {
                let _ = writeln!(s, "{0}::{0}() {{}}", r.name);
            }
            if r.dtor.is_some() {
                let _ = writeln!(s, "{0}::~{0}() {{}}", r.name);
            }
            for m in &r.methods {
                if !m.pure && m.kind == MethodKind::Normal {
                    let head = self.method_head(m, &format!("{}::", r.name));
                    let _ = writeln!(s, "{} {{}}", self.declarator(&m.ret, &head));
                }
            }
        }
    }

    /// `[qual]name(params) [const]`.
    fn method_head(&self, m: &MethodDef, qual: &str) -> String {
        let params: Vec<String> = m.params.iter().map(|p| self.declarator(p, "")).collect();
        let c = if m.is_const { " const" } else { "" };
        format!("{qual}{}({}){c}", m.name, params.join(", "))
    }

    // ------------------------------------------------------------------
    // kiln AST
    // ------------------------------------------------------------------

    /// Builds the model as a kiln AST for `target`; returns the AST and
    /// the declaration of each item.
    pub fn ast(&self, target: &Target) -> (Ast, Vec<DeclId>) {
        let mut ast = Ast::new(Arc::new(Interner::new()), TypeContext::new(target.types.clone()));
        let mut decls: Vec<DeclId> = Vec::new();
        let mut methods: HashMap<(usize, usize), DeclId> = HashMap::new();
        for item in &self.items {
            let d = match item {
                Item::Enum(e) => {
                    let h = head(&ast, ast.tu, &e.name);
                    let int = builtin(BuiltinType::Int);
                    let fixed = e.fixed.map(|b| {
                        let q = builtin(b);
                        (q, q)
                    });
                    let d = ast.builder().enumeration(h, 0, fixed, None);
                    ast.builder().begin_enum_definition(d).expect("enum def");
                    let computed = if fixed.is_some() { None } else { Some((int, int)) };
                    ast.builder().complete_enum_definition(d, computed).expect("enum complete");
                    d
                }
                Item::Typedef(t) => {
                    let mut ty = lower(&mut ast, &decls, &t.ty);
                    if let Some(n) = t.vector_bytes {
                        let elem_size = target.builtin_size_align(match t.ty {
                            Ty::B(b) => b,
                            _ => panic!("vector of non-builtin"),
                        });
                        let count = u32::try_from(n / elem_size.0).expect("count");
                        ty = ast.types.vector(ty, count, VectorKind::Generic);
                    }
                    let h = head(&ast, ast.tu, &t.name);
                    let d = ast.builder().typedef(h, DeclKind::Typedef, ty, None);
                    if let Some(n) = t.aligned {
                        let a = aligned_attr(&mut ast, Some(n), AttrSyntax::Gnu);
                        ast.builder().attach_attrs(d, &[a]).expect("attrs");
                    }
                    d
                }
                Item::Record(r) => self.record_ast(&mut ast, &decls, &mut methods, r),
            };
            decls.push(d);
        }
        (ast, decls)
    }

    #[allow(clippy::too_many_lines, reason = "one linear construction of a class definition")]
    fn record_ast(
        &self,
        ast: &mut Ast,
        decls: &[DeclId],
        methods: &mut HashMap<(usize, usize), DeclId>,
        r: &RecDef,
    ) -> DeclId {
        let idx = decls.len();
        let h = head(ast, ast.tu, &r.name);
        let d = ast.builder().record(h, r.tag, None);
        let mut attrs = Vec::new();
        if r.packed {
            attrs.push(simple_attr(ast, AttrKind::PACKED));
        }
        if let Some(n) = r.aligned {
            attrs.push(aligned_attr(ast, (n != 0).then_some(n), AttrSyntax::Gnu));
        }
        if let Some(n) = r.alignas {
            attrs.push(aligned_attr(ast, Some(n), AttrSyntax::Keyword));
        }
        if !attrs.is_empty() {
            ast.builder().attach_attrs(d, &attrs).expect("attrs");
        }
        if let Some(n) = r.pack {
            ast.decls.cold.record_pack.insert(d, n).expect("pack");
        }
        ast.builder().begin_record_definition(d).expect("def");
        if !r.bases.is_empty() {
            let specs: Vec<BaseSpec> = r
                .bases
                .iter()
                .map(|&i| BaseSpec {
                    ty: ast.decls.get(decls[i]).ty().expect("base type"),
                    loc: L,
                    is_virtual: r.is_virtual_base(i),
                    access: Access::Public,
                    pack_expansion: false,
                })
                .collect();
            ast.decls.set_bases(d, &specs, &mut ast.lists).expect("bases");
        }
        for f in &r.fields {
            let ty = lower(ast, decls, f.ty());
            let name = match &f.name {
                Some(n) => DeclName::ident(ast.symbols().intern(n)),
                None => DeclName::EMPTY,
            };
            let access = if f.private { Access::Private } else { Access::Public };
            let width = f.bits.map(|w| {
                ast.builder().int_literal(builtin(BuiltinType::Int), w, Radix::Decimal, L)
            });
            let flags = if f.nua { FieldData::NO_UNIQUE_ADDRESS } else { 0 };
            let fd =
                ast.builder().field(DeclHead::new(d, name, L).access(access), ty, width, flags);
            let mut fattrs = Vec::new();
            if f.packed {
                fattrs.push(simple_attr(ast, AttrKind::PACKED));
            }
            if let Some(n) = f.aligned {
                fattrs.push(aligned_attr(ast, Some(n), AttrSyntax::Gnu));
            }
            if let Some(n) = f.alignas {
                fattrs.push(aligned_attr(ast, Some(n), AttrSyntax::Keyword));
            }
            if !fattrs.is_empty() {
                ast.builder().attach_attrs(fd, &fattrs).expect("attrs");
            }
        }
        if r.user_ctor {
            let class: TypeId = ast.decls.get(d).ty().expect("record type").ty();
            let name = ast.names.constructor(class);
            let fty = ast.types.function(
                builtin(BuiltinType::Void),
                &[],
                FuncInfo::new(),
                ExcSpec::default(),
            );
            let h = DeclHead::new(d, name, L).access(Access::Public);
            ast.builder().function(h, DeclKind::CXXConstructor, fty, FnSpec::new(), None);
        }
        // Member functions may name the class itself.
        let mut self_decls = decls.to_vec();
        self_decls.push(d);
        let class: TypeId = ast.decls.get(d).ty().expect("record type").ty();
        let dtor_ty = ast.types.function(
            builtin(BuiltinType::Void),
            &[],
            FuncInfo::new(),
            ExcSpec::default(),
        );
        let declare_dtor = |ast: &mut Ast,
                            methods: &mut HashMap<(usize, usize), DeclId>,
                            written: Option<DtorDef>| {
            let name = ast.names.destructor(class);
            let mut h = DeclHead::new(d, name, L).access(Access::Public);
            if written.is_none() {
                h = h.flags(DeclFlags::IMPLICIT);
            }
            let spec = FnSpec::new()
                .with_virtual(r.dtor_virtual)
                .with_virtual_written(written.is_some_and(|w| w.virtual_written))
                .with_pure(written.is_some_and(|w| w.pure));
            let m = ast.builder().function(h, DeclKind::CXXDestructor, dtor_ty, spec, None);
            let over: Vec<DeclId> =
                self.overridden_dtors(&r.bases).iter().map(|k| methods[k]).collect();
            ast.builder().set_overrides(m, &over).expect("overrides");
            methods.insert((idx, DTOR), m);
        };
        for k in 0..=r.methods.len() {
            if let Some(w) = r.dtor.filter(|w| w.pos == k) {
                declare_dtor(ast, methods, Some(w));
            }
            let Some(md) = r.methods.get(k) else { break };
            let ret = lower(ast, &self_decls, &md.ret);
            let params: Vec<QualType> =
                md.params.iter().map(|p| lower(ast, &self_decls, p)).collect();
            let quals = if md.is_const { Quals::CONST } else { Quals::from_bits(0) };
            let fty = ast.types.function(
                ret,
                &params,
                FuncInfo::new().with_this_quals(quals),
                ExcSpec::default(),
            );
            let mut spec = FnSpec::new()
                .with_virtual(md.is_virtual)
                .with_virtual_written(md.virtual_written)
                .with_pure(md.pure)
                .with_deleted(md.kind == MethodKind::Deleted);
            if md.kind == MethodKind::Consteval {
                spec = spec.with_constexpr_kind(ConstexprKind::Consteval);
            }
            let h = head(ast, d, &md.name).access(Access::Public);
            let m = ast.builder().function(h, DeclKind::CXXMethod, fty, spec, None);
            if md.is_virtual {
                let over: Vec<DeclId> =
                    self.overridden(&r.bases, md).iter().map(|k| methods[k]).collect();
                ast.builder().set_overrides(m, &over).expect("overrides");
            }
            methods.insert((idx, k), m);
        }
        if r.dtor.is_none() && r.dtor_virtual {
            declare_dtor(ast, methods, None);
        }
        let mut bits = DefinitionBits::empty();
        if r.polymorphic {
            bits = bits.with(DefinitionBits::POLYMORPHIC);
        }
        if r.has_vbases {
            bits = bits.with(DefinitionBits::HAS_VIRTUAL_BASES);
        }
        ast.builder().complete_record_definition(d, bits);
        d
    }
}

fn head(ast: &Ast, ctx: DeclId, s: &str) -> DeclHead {
    DeclHead::new(ctx, DeclName::ident(ast.symbols().intern(s)), L)
}

pub fn builtin(b: BuiltinType) -> QualType {
    b.qual_type()
}

fn simple_attr(ast: &mut Ast, kind: AttrKind) -> AttrId {
    ast.attrs.push(AttrNode { kind, syntax: AttrSyntax::Gnu, flags: 0, loc: L, a: 0, b: 0 })
}

fn aligned_attr(ast: &mut Ast, n: Option<u64>, syntax: AttrSyntax) -> AttrId {
    let a = n.map_or(0, |n| {
        let e: ExprId = ast.builder().int_literal(builtin(BuiltinType::Int), n, Radix::Decimal, L);
        e.raw()
    });
    ast.attrs.push(AttrNode { kind: AttrKind::ALIGNED, syntax, flags: 0, loc: L, a, b: 0 })
}

/// The kiln type of model type `t`.
fn lower(ast: &mut Ast, decls: &[DeclId], t: &Ty) -> QualType {
    match t {
        Ty::B(b) => builtin(*b),
        Ty::BitInt(signed, w) => ast.types.bit_int(*signed, *w),
        Ty::Ptr(p) => {
            let p = lower(ast, decls, p);
            ast.types.pointer_to(p)
        }
        Ty::LRef(p) => {
            let p = lower(ast, decls, p);
            ast.types.lvalue_ref_to(p)
        }
        Ty::RRef(p) => {
            let p = lower(ast, decls, p);
            ast.types.rvalue_ref_to(p)
        }
        Ty::Arr(e, n) => {
            let e = lower(ast, decls, e);
            let size = ast.builder().int_literal(builtin(BuiltinType::Int), *n, Radix::Decimal, L);
            ast.types.constant_array(e, *n, Some(size))
        }
        Ty::Flex(e) => {
            let e = lower(ast, decls, e);
            ast.types.incomplete_array(e)
        }
        Ty::Named(i) => ast.decls.get(decls[*i]).ty().expect("named type"),
        Ty::Complex(e) => {
            let e = lower(ast, decls, e);
            ast.types.complex(e)
        }
        Ty::Atomic(e) => {
            let e = lower(ast, decls, e);
            ast.types.atomic(e)
        }
        Ty::Cv(e, q) => {
            let e = lower(ast, decls, e);
            ast.types.qualify(e, Quals::from_bits(*q))
        }
        Ty::MemPtr(p, c) => {
            let p = lower(ast, decls, p);
            let class = ast.decls.get(decls[*c]).ty().expect("class type").ty();
            ast.types.member_pointer(p, class)
        }
        Ty::Func(r, ps) => {
            let r = lower(ast, decls, r);
            let ps: Vec<QualType> = ps.iter().map(|p| lower(ast, decls, p)).collect();
            ast.types.function(r, &ps, FuncInfo::new(), ExcSpec::default())
        }
        Ty::NullptrT => {
            let np = builtin(BuiltinType::NullPtr);
            let e = ast.builder().nullptr_literal(np, L);
            ast.types.decltype_type(e, np, None)
        }
    }
}

// ----------------------------------------------------------------------
// clang
// ----------------------------------------------------------------------

/// The reference compiler.
pub const CLANG: &str = "clang++-19";
/// The line clang prints before each dumped class.
pub const MARKER: &str = "\n*** Dumping AST Record Layout\n";

/// `clang++-19` is installed.
pub fn clang_available() -> bool {
    Command::new(CLANG)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Runs `clang++-19 -fsyntax-only -Xclang -fdump-record-layouts` on
/// `src`; returns stdout, or stderr on failure.
pub fn run_clang(triple: &str, src: &str) -> Result<String, String> {
    run_clang_with(triple, src, &["-fsyntax-only", "-Xclang", "-fdump-record-layouts"])
}

/// Runs clang's code generator (no output file) with
/// `-fdump-vtable-layouts`, so every emitted vtable, and the
/// construction vtables of every emitted VTT, are dumped.
pub fn run_clang_vtables(triple: &str, src: &str) -> Result<String, String> {
    run_clang_with(
        triple,
        src,
        &["-S", "-emit-llvm", "-o", "/dev/null", "-Xclang", "-fdump-vtable-layouts"],
    )
}

fn run_clang_with(triple: &str, src: &str, mode: &[&str]) -> Result<String, String> {
    let mut child = Command::new(CLANG)
        .args(["-std=c++2c", "-w"])
        .args(mode)
        .arg(format!("--target={triple}"))
        .args(["-x", "c++", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child.stdin.take().expect("stdin").write_all(src.as_bytes()).map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Dump blocks (without the marker) keyed by their first line, the class
/// header.
pub fn blocks(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for b in text.split(MARKER).skip(1) {
        let head = b.lines().next().unwrap_or_default().to_owned();
        map.insert(head, b.to_owned());
    }
    map
}

/// True for the first line of a vtable dump.
pub fn is_vtable_header(line: &str) -> bool {
    line.starts_with("Vtable for '") || line.starts_with("Construction vtable for ('")
}

/// clang's `-fdump-vtable-layouts` output split into one block per
/// vtable, keyed by its header line. The "Original map" section clang
/// prints before each block (its own cache of overridden methods, not
/// part of the layout) is dropped.
pub fn vtable_blocks(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut cur: Option<(String, String)> = None;
    let mut skipping = false;
    for line in text.lines() {
        if line == "Original map" || is_vtable_header(line) {
            if let Some((h, b)) = cur.take() {
                map.insert(h, b);
            }
            skipping = line == "Original map";
            if skipping {
                continue;
            }
            cur = Some((line.to_owned(), String::new()));
        } else if skipping && line.starts_with(' ') {
            continue;
        }
        skipping = false;
        if let Some((_, b)) = &mut cur {
            b.push_str(line);
            b.push('\n');
        }
    }
    if let Some((h, b)) = cur.take() {
        map.insert(h, b);
    }
    map
}
