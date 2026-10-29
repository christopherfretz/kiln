//! A tiny class-definition model shared by the `kiln_abi` tests: each
//! [`Model`] renders both as C++ source (for clang) and as a kiln AST
//! built with `AstBuilder` (for `kiln_abi`), so the two layouts can be
//! compared. Items are declared in order; a type may only name earlier
//! items.
#![allow(dead_code, reason = "each test binary uses a subset")]

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::process::{Command, Stdio};
use std::sync::Arc;

use kiln_abi::target::Target;
use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::{Access, BaseSpec, DeclKind, DefinitionBits, FieldData, FnSpec, TagKind};
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

/// A class definition.
#[derive(Clone, Debug)]
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
    /// Non-virtual public base classes: model item indices, in order.
    pub bases: Vec<usize>,
    /// `#pragma pack(push, N)` in effect at the definition (bytes).
    pub pack: Option<u16>,
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
            pack: None,
        }
    }
}

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
    pub fn push(&mut self, item: Item) -> usize {
        self.items.push(item);
        self.items.len() - 1
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
        let bases: Vec<String> =
            r.bases.iter().map(|&i| format!("public {}", self.name_of(i))).collect();
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
            let _ = writeln!(s, "public: {}() {{}}", r.name);
        }
        let _ = writeln!(s, "}};");
        if r.pack.is_some() {
            let _ = writeln!(s, "#pragma pack(pop)");
        }
    }

    // ------------------------------------------------------------------
    // kiln AST
    // ------------------------------------------------------------------

    /// Builds the model as a kiln AST for `target`; returns the AST and
    /// the declaration of each item.
    pub fn ast(&self, target: &Target) -> (Ast, Vec<DeclId>) {
        let mut ast = Ast::new(Arc::new(Interner::new()), TypeContext::new(target.types.clone()));
        let mut decls: Vec<DeclId> = Vec::new();
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
                Item::Record(r) => Self::record_ast(&mut ast, &decls, r),
            };
            decls.push(d);
        }
        (ast, decls)
    }

    fn record_ast(ast: &mut Ast, decls: &[DeclId], r: &RecDef) -> DeclId {
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
                    is_virtual: false,
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
        ast.builder().complete_record_definition(d, DefinitionBits::empty());
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
    let mut child = Command::new(CLANG)
        .args(["-std=c++2c", "-fsyntax-only", "-w", "-Xclang", "-fdump-record-layouts"])
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
