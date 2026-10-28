//! The parser and sema-lite of the test DSL: a C subset (the Oven's M2
//! subset) lowered to clang-shaped kiln ASTs with `AstBuilder`. Implicit
//! conversions follow [conv] and [expr.arith.conv] for x86-64 Linux.
//! Test code: malformed input panics with the offending offset.

use std::collections::HashMap;
use std::sync::Arc;

use kiln_abi::Target;
use kiln_ast::consts::{FloatKind, IntBits, Scalar};
use kiln_ast::decl::{
    ConstexprKind, DeclKind, DefinitionBits, FnSpec, TagKind, VarSpec, VarStorage,
};
use kiln_ast::expr::{
    BinaryOp, CastKind, CharKind, ExprFlags, ExprKind, NonOdrUse, ObjectKind, Radix, StringKind,
    TraitKind, UnaryOp, ValueCategory,
};
use kiln_ast::names::DeclName;
use kiln_ast::stmt::{CaseData, IfData, IfKind, LoopData, SwitchData};
use kiln_ast::strings::StringEncoding;
use kiln_ast::types::{BuiltinType as B, ExcSpec, FuncInfo, Quals, TypeContext, TypeView};
use kiln_ast::{Ast, ConstValueId, DeclHead, DeclId, ExprId, QualType, StmtId};
use kiln_oven::{Budget, Oven};
use kiln_source::Loc;
use kiln_source::symbol::Interner;

use super::Bridge;
use super::lex::{Tok, Token, decode, lex};

/// A top-level construct the harness evaluates, in source order.
#[derive(Clone, Copy, Debug)]
pub enum Item {
    /// `static_assert(cond)`.
    Assert { cond: ExprId, loc: Loc },
    /// A namespace-scope or static local variable with an initializer.
    Var { decl: DeclId, init: ExprId, loc: Loc, constexpr: bool },
}

#[derive(Clone, Copy, Debug)]
enum Sym {
    Var(DeclId, QualType),
    Func(DeclId, QualType),
    Enumerator(DeclId, QualType),
    Type(QualType),
}

#[derive(Clone, Debug)]
struct Field {
    name: Option<String>,
    decl: DeclId,
    ty: QualType,
    width: Option<u32>,
}

#[derive(Clone, Debug)]
enum DOp {
    Ptr(bool),
    Arr(Option<u64>),
    Func(Vec<Param>),
}

#[derive(Clone, Debug)]
struct Param {
    name: Option<String>,
    loc: Loc,
    ty: QualType,
}

#[derive(Default, Clone, Copy)]
struct Spec {
    ty: Option<QualType>,
    constexpr: bool,
    consteval: bool,
    constinit: bool,
    is_const: bool,
    is_static: bool,
    is_extern: bool,
    is_typedef: bool,
}

enum Init {
    E(ExprId),
    List(Vec<Init>, Loc, Loc),
    Desig(String, Box<Init>),
}

/// The parsed translation unit.
pub struct Parsed {
    pub ast: Ast,
    pub items: Vec<Item>,
    pub names: HashMap<String, DeclId>,
}

pub struct Parser<'t> {
    toks: Vec<Token>,
    pos: usize,
    pub ast: Ast,
    target: &'t Target,
    scopes: Vec<HashMap<String, Sym>>,
    tags: HashMap<String, QualType>,
    fields: HashMap<DeclId, Vec<Field>>,
    bitfields: HashMap<DeclId, u32>,
    enums: HashMap<DeclId, (QualType, QualType, bool)>,
    func: Option<DeclId>,
    ret: QualType,
    switches: Vec<(QualType, Vec<StmtId>)>,
    fold: Oven,
    items: Vec<Item>,
    names: HashMap<String, DeclId>,
}

fn loc_of(at: usize) -> Loc {
    Loc::from_raw(u32::try_from(at).expect("offset") + 1)
}

/// Parses `src` for `target`.
pub fn parse(src: &str, target: &Target) -> Parsed {
    let ast = Ast::new(Arc::new(Interner::new()), TypeContext::new(target.types.clone()));
    let mut p = Parser {
        toks: lex(src),
        pos: 0,
        ast,
        target,
        scopes: vec![HashMap::new()],
        tags: HashMap::new(),
        fields: HashMap::new(),
        bitfields: HashMap::new(),
        enums: HashMap::new(),
        func: None,
        ret: QualType::VOID,
        switches: Vec::new(),
        fold: Oven::new(Budget::default()),
        items: Vec::new(),
        names: HashMap::new(),
    };
    while p.peek() != &Tok::Eof {
        p.external_decl();
    }
    Parsed { ast: p.ast, items: p.items, names: p.names }
}

const TYPE_WORDS: &[&str] = &[
    "void", "bool", "char", "short", "int", "long", "signed", "unsigned", "float", "double",
    "__int128", "char8_t", "char16_t", "char32_t", "wchar_t", "struct", "union", "enum", "const",
    "volatile", "_BitInt",
];
const SPEC_WORDS: &[&str] =
    &["constexpr", "consteval", "constinit", "static", "extern", "inline", "typedef"];

impl Parser<'_> {
    // ---- tokens -------------------------------------------------------------

    fn peek(&self) -> &Tok {
        &self.toks[self.pos].tok
    }
    fn peek_at(&self, n: usize) -> &Tok {
        &self.toks[(self.pos + n).min(self.toks.len() - 1)].tok
    }
    fn at(&self) -> usize {
        self.toks[self.pos].at
    }
    fn loc(&self) -> Loc {
        loc_of(self.at())
    }
    fn next(&mut self) -> Token {
        let t = self.toks[self.pos].clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        t
    }
    fn is(&self, p: &str) -> bool {
        match self.peek() {
            Tok::Punct(q) => *q == p,
            Tok::Ident(w) => w == p,
            _ => false,
        }
    }
    fn eat(&mut self, p: &str) -> bool {
        if self.is(p) {
            self.next();
            true
        } else {
            false
        }
    }
    fn expect(&mut self, p: &str) -> Loc {
        assert!(self.is(p), "expected `{p}` at offset {}, found {:?}", self.at(), self.peek());
        let l = self.loc();
        self.next();
        l
    }
    fn ident(&mut self) -> (String, Loc) {
        let l = self.loc();
        match self.next().tok {
            Tok::Ident(s) => (s, l),
            t => panic!("expected an identifier at {l:?}, found {t:?}"),
        }
    }

    // ---- scopes -------------------------------------------------------------

    fn lookup(&self, name: &str) -> Option<Sym> {
        self.scopes.iter().rev().find_map(|s| s.get(name).copied())
    }
    fn bind(&mut self, name: &str, s: Sym) {
        self.scopes.last_mut().expect("scope").insert(name.to_owned(), s);
    }
    fn starts_type(&self) -> bool {
        match self.peek() {
            Tok::Ident(w) => {
                TYPE_WORDS.contains(&w.as_str())
                    || SPEC_WORDS.contains(&w.as_str())
                    || matches!(self.lookup(w), Some(Sym::Type(_)))
            }
            _ => false,
        }
    }
    fn ctx(&self) -> DeclId {
        self.func.unwrap_or(self.ast.tu)
    }
    fn name(&self, s: &str) -> DeclName {
        DeclName::ident(self.ast.symbols().intern(s))
    }

    // ---- types --------------------------------------------------------------

    fn canon(&self, q: QualType) -> QualType {
        self.ast.types.canonical(q)
    }
    fn view(&self, q: QualType) -> TypeView {
        self.ast.types.canonical_view(q)
    }
    fn unqual(&mut self, q: QualType) -> QualType {
        let c = self.canon(q);
        self.ast.types.unqualified(c)
    }
    fn ety(&self, e: ExprId) -> QualType {
        self.ast.exprs.get(e).ty()
    }
    fn vk(&self, e: ExprId) -> ValueCategory {
        self.ast.exprs.get(e).value_category()
    }
    fn builtin(&self, q: QualType) -> Option<B> {
        match self.view(q) {
            TypeView::Builtin(b) => Some(b),
            _ => None,
        }
    }
    /// `(width, signed, rank)` of an integral or enumeration type.
    fn int_info(&self, q: QualType) -> Option<(u32, bool, u32)> {
        match self.view(q) {
            TypeView::Builtin(b) => Some(match b {
                B::Bool => (1, false, 0),
                B::Char | B::SChar => (8, true, 1),
                B::UChar | B::Char8 => (8, false, 1),
                B::Char16 => (16, false, 2),
                B::Short => (16, true, 2),
                B::UShort => (16, false, 2),
                B::Char32 => (32, false, 3),
                B::WChar | B::Int => (32, true, 3),
                B::UInt => (32, false, 3),
                B::Long => (64, true, 4),
                B::ULong => (64, false, 4),
                B::LongLong => (64, true, 5),
                B::ULongLong => (64, false, 5),
                B::Int128 => (128, true, 6),
                B::UInt128 => (128, false, 6),
                _ => return None,
            }),
            TypeView::Enum(d) => self.enums.get(&d).and_then(|e| self.int_info(e.0)),
            TypeView::BitInt { signed, width } => Some((width, signed, 3)),
            _ => None,
        }
    }
    fn is_int(&self, q: QualType) -> bool {
        self.int_info(q).is_some()
    }
    fn is_scoped_enum(&self, q: QualType) -> bool {
        matches!(self.view(q), TypeView::Enum(d) if self.enums.get(&d).is_some_and(|e| e.2))
    }
    fn float_rank(&self, q: QualType) -> Option<u32> {
        match self.builtin(q)? {
            B::Float16 => Some(0),
            B::Float => Some(1),
            B::Double => Some(2),
            B::LongDouble => Some(3),
            B::Float128 => Some(4),
            _ => None,
        }
    }
    fn is_float(&self, q: QualType) -> bool {
        self.float_rank(q).is_some()
    }
    fn pointee(&self, q: QualType) -> Option<QualType> {
        match self.view(q) {
            TypeView::Pointer(p) => Some(p),
            _ => None,
        }
    }
    fn is_ptr(&self, q: QualType) -> bool {
        self.pointee(q).is_some()
    }
    fn is_nullptr_t(&self, q: QualType) -> bool {
        self.builtin(q) == Some(B::NullPtr)
    }
    fn is_bool(&self, q: QualType) -> bool {
        self.builtin(q) == Some(B::Bool)
    }
    fn is_record(&self, q: QualType) -> Option<DeclId> {
        match self.view(q) {
            TypeView::Record(d) => Some(d),
            _ => None,
        }
    }
    fn array_of(&self, q: QualType) -> Option<(QualType, Option<u64>)> {
        match self.view(q) {
            TypeView::ConstantArray(a) => Some((a.elem, Some(a.size))),
            TypeView::IncompleteArray(e) => Some((e, None)),
            _ => None,
        }
    }
    fn is_aggregate(&self, q: QualType) -> bool {
        self.array_of(q).is_some() || self.is_record(q).is_some()
    }
    fn is_char_like(&self, q: QualType) -> bool {
        matches!(
            self.builtin(q),
            Some(B::Char | B::SChar | B::UChar | B::Char8 | B::Char16 | B::Char32 | B::WChar)
        )
    }
    fn unsigned_of(q: QualType) -> QualType {
        match q {
            QualType::INT => QualType::UINT,
            QualType::LONG => QualType::ULONG,
            QualType::LONGLONG => QualType::ULONGLONG,
            QualType::INT128 => QualType::UINT128,
            q => q,
        }
    }
    fn add_const(&mut self, q: QualType) -> QualType {
        self.ast.types.qualify(q, Quals::CONST)
    }
    fn is_const(&self, q: QualType) -> bool {
        self.ast.types.effective_quals(q).contains(Quals::CONST)
    }

    // ---- declarations -------------------------------------------------------

    fn spec(&mut self) -> Spec {
        let mut s = Spec::default();
        let (mut longs, mut short, mut unsigned, mut signed) = (0, false, false, false);
        let mut base: Option<&'static str> = None;
        loop {
            let Tok::Ident(w) = self.peek().clone() else { break };
            match w.as_str() {
                "constexpr" => s.constexpr = true,
                "consteval" => s.consteval = true,
                "constinit" => s.constinit = true,
                "static" => s.is_static = true,
                "extern" => s.is_extern = true,
                "typedef" => s.is_typedef = true,
                "inline" | "volatile" => {}
                "const" => s.is_const = true,
                "long" => longs += 1,
                "short" => short = true,
                "unsigned" => unsigned = true,
                "signed" => signed = true,
                "void" | "bool" | "char" | "int" | "float" | "double" | "__int128" | "char8_t"
                | "char16_t" | "char32_t" | "wchar_t" => {
                    base = Some(match w.as_str() {
                        "void" => "void",
                        "bool" => "bool",
                        "char" => "char",
                        "int" => "int",
                        "float" => "float",
                        "double" => "double",
                        "__int128" => "__int128",
                        "char8_t" => "char8_t",
                        "char16_t" => "char16_t",
                        "char32_t" => "char32_t",
                        _ => "wchar_t",
                    });
                }
                "_BitInt" => {
                    self.next();
                    self.expect("(");
                    let Tok::Num(n) = self.next().tok else { panic!("_BitInt width") };
                    self.expect(")");
                    let w: u32 = n.parse().expect("width");
                    s.ty = Some(self.ast.types.bit_int(!unsigned, w));
                    continue;
                }
                "struct" | "union" => {
                    s.ty = Some(self.record_spec());
                    continue;
                }
                "enum" => {
                    s.ty = Some(self.enum_spec());
                    continue;
                }
                _ => {
                    let none = base.is_none() && s.ty.is_none() && longs == 0 && !short;
                    let none = none && !unsigned && !signed;
                    match self.lookup(&w) {
                        Some(Sym::Type(t)) if none => s.ty = Some(t),
                        _ => break,
                    }
                }
            }
            self.next();
        }
        if s.ty.is_none() {
            let b = match base {
                Some("void") => B::Void,
                Some("bool") => B::Bool,
                Some("char") if unsigned => B::UChar,
                Some("char") if signed => B::SChar,
                Some("char") => B::Char,
                Some("float") => B::Float,
                Some("double") if longs > 0 => B::LongDouble,
                Some("double") => B::Double,
                Some("__int128") if unsigned => B::UInt128,
                Some("__int128") => B::Int128,
                Some("char8_t") => B::Char8,
                Some("char16_t") => B::Char16,
                Some("char32_t") => B::Char32,
                Some("wchar_t") => B::WChar,
                _ if short => {
                    if unsigned {
                        B::UShort
                    } else {
                        B::Short
                    }
                }
                _ if longs == 1 => {
                    if unsigned {
                        B::ULong
                    } else {
                        B::Long
                    }
                }
                _ if longs >= 2 => {
                    if unsigned {
                        B::ULongLong
                    } else {
                        B::LongLong
                    }
                }
                _ if unsigned => B::UInt,
                Some(_) => B::Int,
                None if signed => B::Int,
                None => return s,
            };
            s.ty = Some(b.qual_type());
        }
        if s.is_const {
            let t = s.ty.expect("type");
            s.ty = Some(self.add_const(t));
        }
        s
    }

    fn record_spec(&mut self) -> QualType {
        let union = self.ident().0 == "union";
        let name = if let Tok::Ident(_) = self.peek() { Some(self.ident()) } else { None };
        if let Some((n, _)) = &name
            && !self.is("{")
        {
            if let Some(&t) = self.tags.get(n) {
                return t;
            }
        }
        let (n, l) = name.unwrap_or_else(|| (String::new(), self.loc()));
        let h = DeclHead::new(self.ast.tu, self.name(&n), l);
        let tag = if union { TagKind::Union } else { TagKind::Struct };
        let d = self.ast.builder().record(h, tag, None);
        let ty = self.ast.decls.get(d).ty().expect("record type");
        if !n.is_empty() {
            self.tags.insert(n.clone(), ty);
            self.scopes[0].insert(n, Sym::Type(ty));
        }
        if !self.eat("{") {
            return ty;
        }
        self.ast.builder().begin_record_definition(d).expect("definition");
        let mut fields = Vec::new();
        while !self.eat("}") {
            let s = self.spec();
            let base = s.ty.expect("member type");
            loop {
                let (name, ops) = if self.is(":") { (None, Vec::new()) } else { self.declarator() };
                let fty = self.apply(base, &ops);
                let width = if self.eat(":") {
                    let e = self.cond_expr();
                    let e = self.rvalue(e);
                    let v = self.fold_int(e);
                    let ce = self.ast.builder().constant_expr(e, None);
                    Some((u32::try_from(v).expect("width"), ce))
                } else {
                    None
                };
                let (fname, floc) = match &name {
                    Some((n, l)) => (self.name(n), *l),
                    None => (DeclName::EMPTY, self.loc()),
                };
                let fd = self.ast.builder().field(
                    DeclHead::new(d, fname, floc),
                    fty,
                    width.map(|w| w.1),
                    0,
                );
                if let Some((w, _)) = width {
                    self.bitfields.insert(fd, w);
                }
                fields.push(Field {
                    name: name.map(|n| n.0),
                    decl: fd,
                    ty: fty,
                    width: width.map(|w| w.0),
                });
                if !self.eat(",") {
                    break;
                }
            }
            self.expect(";");
        }
        self.ast.builder().complete_record_definition(d, DefinitionBits::empty());
        kiln_abi::ensure_layout(&mut self.ast, d, self.target);
        self.fields.insert(d, fields);
        ty
    }

    fn enum_spec(&mut self) -> QualType {
        self.expect("enum");
        let scoped = self.eat("class") || self.eat("struct");
        let (n, l) = if let Tok::Ident(_) = self.peek() {
            self.ident()
        } else {
            (String::new(), self.loc())
        };
        if !self.is("{")
            && !self.is(":")
            && let Some(&t) = self.tags.get(&n)
        {
            return t;
        }
        let fixed = if self.eat(":") {
            let s = self.spec();
            Some(s.ty.expect("underlying type"))
        } else if scoped {
            Some(QualType::INT)
        } else {
            None
        };
        let fixed_pair = fixed.map(|q| {
            let p = match self.int_info(q) {
                Some((w, _, _)) if w < 32 => QualType::INT,
                _ => q,
            };
            (q, p)
        });
        let h = DeclHead::new(self.ast.tu, self.name(&n), l);
        let flags = if scoped { kiln_ast::decl::EnumData::SCOPED } else { 0 }
            | if fixed.is_some() { kiln_ast::decl::EnumData::FIXED } else { 0 };
        let d = self.ast.builder().enumeration(h, flags, fixed_pair, None);
        let ty = self.ast.decls.get(d).ty().expect("enum type");
        if !n.is_empty() {
            self.tags.insert(n.clone(), ty);
            self.scopes[0].insert(n.clone(), Sym::Type(ty));
        }
        let under = fixed.unwrap_or(QualType::INT);
        let promo = fixed_pair.map_or(QualType::INT, |p| p.1);
        self.enums.insert(d, (under, promo, scoped));
        if !self.eat("{") {
            return ty;
        }
        self.ast.builder().begin_enum_definition(d).expect("enum definition");
        let (w, s, _) = self.int_info(under).expect("integral");
        let mut next: i128 = 0;
        let mut consts = HashMap::new();
        while !self.eat("}") {
            let (en, el) = self.ident();
            let init = if self.eat("=") {
                let e = self.cond_expr();
                let e = self.rvalue(e);
                let v = self.fold_int(e);
                next = v;
                Some(self.ast.builder().constant_expr(e, None))
            } else {
                None
            };
            let enm = self.name(&en);
            let ed = self.ast.builder().enumerator(DeclHead::new(d, enm, el), init);
            let bits =
                if s { IntBits::from_i128(w, next) } else { IntBits::from_u128(w, next as u128) };
            let cv = self
                .ast
                .consts
                .push_scalar(Scalar::Int { ty, signed: s, bits })
                .expect("enumerator value");
            self.ast.decls.set_enumerator_value(ed, cv).expect("value");
            consts.insert(en.clone(), ed);
            if scoped {
                self.names.insert(format!("{n}::{en}"), ed);
            } else {
                self.bind(&en, Sym::Enumerator(ed, ty));
            }
            next += 1;
            if !self.eat(",") {
                self.expect("}");
                break;
            }
        }
        let computed = if fixed.is_some() { None } else { Some((QualType::INT, QualType::INT)) };
        self.ast.builder().complete_enum_definition(d, computed).expect("enum complete");
        ty
    }

    /// A (possibly abstract) declarator: the name and the type operations,
    /// in the order they apply to the base type.
    fn declarator(&mut self) -> (Option<(String, Loc)>, Vec<DOp>) {
        let mut ptrs = Vec::new();
        while self.eat("*") {
            let mut c = false;
            while self.is("const") || self.is("volatile") {
                c |= self.ident().0 == "const";
            }
            ptrs.push(DOp::Ptr(c));
        }
        let mut name = None;
        let mut inner = Vec::new();
        if self.is("(") && matches!(self.peek_at(1), Tok::Punct("*")) {
            self.next();
            let (n, ops) = self.declarator();
            self.expect(")");
            name = n;
            inner = ops;
        } else if let Tok::Ident(w) = self.peek()
            && !TYPE_WORDS.contains(&w.as_str())
        {
            name = Some(self.ident());
        }
        let mut suffixes = Vec::new();
        loop {
            if self.eat("[") {
                if self.eat("]") {
                    suffixes.push(DOp::Arr(None));
                } else {
                    let e = self.cond_expr();
                    let e = self.rvalue(e);
                    let n = self.fold_int(e);
                    self.expect("]");
                    suffixes.push(DOp::Arr(Some(u64::try_from(n).expect("array bound"))));
                }
            } else if self.is("(") {
                self.next();
                suffixes.push(DOp::Func(self.params()));
            } else {
                break;
            }
        }
        suffixes.reverse();
        ptrs.extend(suffixes);
        ptrs.extend(inner);
        (name, ptrs)
    }

    fn params(&mut self) -> Vec<Param> {
        let mut ps = Vec::new();
        if self.eat(")") {
            return ps;
        }
        if self.is("void") && matches!(self.peek_at(1), Tok::Punct(")")) {
            self.next();
            self.next();
            return ps;
        }
        loop {
            let l = self.loc();
            let s = self.spec();
            let (n, ops) = self.declarator();
            let t = self.apply(s.ty.expect("parameter type"), &ops);
            let t = self.adjust_param(t);
            ps.push(Param {
                name: n.as_ref().map(|n| n.0.clone()),
                loc: n.map_or(l, |n| n.1),
                ty: t,
            });
            if !self.eat(",") {
                break;
            }
        }
        self.expect(")");
        ps
    }

    /// [dcl.fct]/5: arrays and functions decay; top-level cv is dropped
    /// from the function type (kept on the parameter).
    fn adjust_param(&mut self, t: QualType) -> QualType {
        if let Some((e, _)) = self.array_of(t) {
            return self.ast.types.pointer_to(e);
        }
        if matches!(self.view(t), TypeView::Function(_)) {
            return self.ast.types.pointer_to(t);
        }
        t
    }

    fn apply(&mut self, base: QualType, ops: &[DOp]) -> QualType {
        let mut t = base;
        for op in ops {
            t = match op {
                DOp::Ptr(c) => {
                    let p = self.ast.types.pointer_to(t);
                    if *c { self.add_const(p) } else { p }
                }
                DOp::Arr(Some(n)) => self.ast.types.constant_array(t, *n, None),
                DOp::Arr(None) => self.ast.types.incomplete_array(t),
                DOp::Func(ps) => {
                    let pts: Vec<QualType> = ps.iter().map(|p| self.unqual(p.ty)).collect();
                    self.ast.types.function(t, &pts, FuncInfo::new(), ExcSpec::default())
                }
            };
        }
        t
    }

    fn type_name(&mut self) -> QualType {
        let s = self.spec();
        let (n, ops) = self.declarator();
        assert!(n.is_none(), "a type-id has no name");
        self.apply(s.ty.expect("type"), &ops)
    }

    fn external_decl(&mut self) {
        if self.eat(";") {
            return;
        }
        if self.is("static_assert") {
            self.static_assert();
            return;
        }
        self.declaration(true);
    }

    fn static_assert(&mut self) {
        let l = self.expect("static_assert");
        self.expect("(");
        let c = self.expr();
        let c = self.to_bool(c);
        if self.eat(",") {
            self.next();
        }
        self.expect(")");
        self.expect(";");
        let ctx = self.ctx();
        self.ast.builder().static_assert(ctx, l, c, None);
        self.items.push(Item::Assert { cond: c, loc: l });
    }

    /// A simple-declaration (or function definition at namespace scope).
    /// Returns the variables declared.
    fn declaration(&mut self, top: bool) -> Vec<DeclId> {
        let s = self.spec();
        let base = s.ty.expect("declaration type");
        let mut out = Vec::new();
        if self.eat(";") {
            return out;
        }
        loop {
            let (name, ops) = self.declarator();
            let (n, l) = name.expect("declarator name");
            if s.is_typedef {
                let t = self.apply(base, &ops);
                self.bind(&n, Sym::Type(t));
            } else if let Some(DOp::Func(ps)) = ops.last() {
                let ps = ps.clone();
                let t = self.apply(base, &ops);
                self.function(&s, &n, l, t, ps);
                if !top || self.toks[self.pos - 1].tok == Tok::Punct("}") {
                    return out;
                }
            } else {
                let mut t = self.apply(base, &ops);
                if s.constexpr {
                    t = self.add_const(t);
                }
                out.push(self.var(&s, &n, l, t));
            }
            if !self.eat(",") {
                break;
            }
        }
        self.expect(";");
        out
    }

    fn function(&mut self, s: &Spec, n: &str, l: Loc, fty: QualType, ps: Vec<Param>) {
        let prev = match self.scopes[0].get(n) {
            Some(Sym::Func(d, _)) => Some(*d),
            _ => None,
        };
        let ck = if s.consteval {
            ConstexprKind::Consteval
        } else if s.constexpr {
            ConstexprKind::Constexpr
        } else {
            ConstexprKind::None
        };
        let spec = FnSpec::new().with_constexpr_kind(ck);
        let h = DeclHead::new(self.ast.tu, self.name(n), l);
        let d = self.ast.builder().function(h, DeclKind::Function, fty, spec, prev);
        self.scopes[0].insert(n.to_owned(), Sym::Func(d, fty));
        self.names.insert(n.to_owned(), d);
        let mut pds = Vec::new();
        for (i, p) in ps.iter().enumerate() {
            let pn = p.name.as_deref().map_or(DeclName::EMPTY, |x| self.name(x));
            let pd =
                self.ast.builder().param(d, pn, p.loc, p.ty, u16::try_from(i).expect("index"), 0);
            pds.push(pd);
        }
        self.ast.builder().set_params(d, &pds).expect("params");
        if !self.is("{") {
            return;
        }
        let ret = match self.view(fty) {
            TypeView::Function(p) => p.ret(),
            _ => unreachable!(),
        };
        let saved = (self.func, self.ret);
        self.func = Some(d);
        self.ret = ret;
        self.scopes.push(HashMap::new());
        for (p, &pd) in ps.iter().zip(&pds) {
            if let Some(pn) = &p.name {
                self.bind(pn, Sym::Var(pd, p.ty));
            }
        }
        let body = self.compound();
        self.scopes.pop();
        (self.func, self.ret) = saved;
        self.ast.builder().set_body(d, body).expect("body");
    }

    fn var(&mut self, s: &Spec, n: &str, l: Loc, mut t: QualType) -> DeclId {
        let global = self.func.is_none() || s.is_static || s.is_extern;
        let storage = if s.is_static {
            VarStorage::Static
        } else if s.is_extern {
            VarStorage::Extern
        } else {
            VarStorage::None
        };
        let spec = VarSpec::new()
            .with_storage(storage)
            .with_constexpr(s.constexpr)
            .with_constinit(s.constinit);
        let has_init = self.is("=") || self.is("{");
        // An array of unknown bound takes its bound from the initializer.
        let mut early = None;
        if has_init && matches!(self.array_of(t), Some((_, None))) {
            self.eat("=");
            let i = self.init_clause();
            let (elem, _) = self.array_of(t).expect("array");
            let n = match &i {
                Init::List(v, ..) => v.len() as u64,
                Init::E(e) => {
                    let st = self.ety(*e);
                    self.array_of(st).and_then(|a| a.1).expect("string literal bound")
                }
                Init::Desig(..) => panic!("designator"),
            };
            t = self.ast.types.constant_array(elem, n, None);
            early = Some(i);
        }
        let ctx = self.ctx();
        let vn = self.name(n);
        let d = self.ast.builder().var(DeclHead::new(ctx, vn, l), t, spec, None);
        self.bind(n, Sym::Var(d, t));
        if self.func.is_none() {
            self.names.insert(n.to_owned(), d);
        }
        let init = if let Some(i) = early {
            Some(self.build_init(t, i))
        } else if has_init {
            self.eat("=");
            let i = self.init_clause();
            Some(self.build_init(t, i))
        } else if self.is_aggregate(t) && !global {
            None
        } else {
            None
        };
        if let Some(i) = init {
            self.ast.builder().set_var_init(d, i).expect("init");
            if global {
                self.items.push(Item::Var { decl: d, init: i, loc: l, constexpr: s.constexpr });
            }
        }
        d
    }

    // ---- initializers -------------------------------------------------------

    fn init_clause(&mut self) -> Init {
        if self.is("{") {
            let lb = self.expect("{");
            let mut v = Vec::new();
            while !self.is("}") {
                if self.is(".") {
                    self.next();
                    let (n, _) = self.ident();
                    self.expect("=");
                    let i = self.init_clause();
                    v.push(Init::Desig(n, Box::new(i)));
                } else {
                    v.push(self.init_clause());
                }
                if !self.eat(",") {
                    break;
                }
            }
            let rb = self.expect("}");
            Init::List(v, lb, rb)
        } else {
            Init::E(self.assign_expr())
        }
    }

    fn build_init(&mut self, ty: QualType, i: Init) -> ExprId {
        match i {
            Init::E(e) => {
                if self.is_aggregate(ty) {
                    if self.is_string_lit(e) {
                        return e;
                    }
                    let r = self.rvalue(e);
                    return r;
                }
                let r = self.rvalue(e);
                self.convert(r, ty)
            }
            Init::List(items, lb, rb) => {
                if self.is_aggregate(ty) {
                    let mut it = items.into_iter().peekable();
                    let e = self.agg(ty, &mut it, lb, rb);
                    assert!(it.next().is_none(), "excess elements in initializer");
                    e
                } else {
                    match items.into_iter().next() {
                        Some(x) => self.build_init(ty, x),
                        None => {
                            let u = self.unqual(ty);
                            self.ast.builder().implicit_value_init(u)
                        }
                    }
                }
            }
            Init::Desig(..) => panic!("a designator outside a braced list"),
        }
    }

    fn is_string_lit(&self, e: ExprId) -> bool {
        let mut e = e;
        while self.ast.exprs.get(e).kind() == ExprKind::ParenExpr {
            e = self.ast.exprs.operand(e);
        }
        self.ast.exprs.get(e).kind() == ExprKind::StringLiteral
    }

    fn agg(
        &mut self,
        ty: QualType,
        it: &mut std::iter::Peekable<std::vec::IntoIter<Init>>,
        lb: Loc,
        rb: Loc,
    ) -> ExprId {
        let uty = self.unqual(ty);
        let quals = self.is_const(ty);
        let mut inits = Vec::new();
        let mut filler = None;
        let mut union_field = None;
        if let Some((elem, n)) = self.array_of(ty) {
            let n = n.expect("bound");
            if self.is_char_like(elem)
                && let Some(Init::E(e)) = it.peek()
                && self.is_string_lit(*e)
            {
                let Some(Init::E(e)) = it.next() else { unreachable!() };
                inits.push(e);
            } else {
                while (inits.len() as u64) < n && it.peek().is_some() {
                    let x = self.sub_init(elem, it);
                    inits.push(x);
                }
                if (inits.len() as u64) < n {
                    let ue = self.unqual(elem);
                    filler = Some(self.ast.builder().implicit_value_init(ue));
                }
            }
        } else if let Some(r) = self.is_record(ty) {
            let fields: Vec<Field> =
                self.fields[&r].iter().filter(|f| f.name.is_some()).cloned().collect();
            let union = self.ast.decls.record(r).is_some_and(|d| d.tag == TagKind::Union);
            let mut fi = 0;
            while fi < fields.len() && it.peek().is_some() {
                if let Some(Init::Desig(name, _)) = it.peek() {
                    fi = fields
                        .iter()
                        .position(|f| f.name.as_deref() == Some(name.as_str()))
                        .unwrap_or_else(|| panic!("no field {name}"));
                    assert!(union || fi >= inits.len(), "designators out of order");
                    let Some(Init::Desig(_, inner)) = it.next() else { unreachable!() };
                    while !union && inits.len() < fi {
                        let f = &fields[inits.len()];
                        let fty = if quals { self.add_const(f.ty) } else { f.ty };
                        let u = self.unqual(fty);
                        let z = self.ast.builder().implicit_value_init(u);
                        inits.push(z);
                    }
                    let fty = if quals { self.add_const(fields[fi].ty) } else { fields[fi].ty };
                    let x = self.build_init(fty, *inner);
                    inits.push(x);
                } else {
                    let fty = if quals { self.add_const(fields[fi].ty) } else { fields[fi].ty };
                    let x = self.sub_init(fty, it);
                    inits.push(x);
                }
                if union {
                    union_field = Some(fields[fi].decl);
                    break;
                }
                fi += 1;
            }
            if union && union_field.is_none() {
                union_field = fields.first().map(|f| f.decl);
            }
        }
        self.ast.builder().init_list(uty, &inits, filler, union_field, lb, rb)
    }

    fn sub_init(
        &mut self,
        ty: QualType,
        it: &mut std::iter::Peekable<std::vec::IntoIter<Init>>,
    ) -> ExprId {
        match it.peek() {
            Some(Init::List(..)) => {
                let x = it.next().expect("item");
                self.build_init(ty, x)
            }
            Some(Init::E(e)) => {
                let e = *e;
                let et = self.unqual(self.ety(e));
                let uty = self.unqual(ty);
                let whole = et == uty
                    || (self.is_string_lit(e)
                        && self.array_of(ty).is_some_and(|(el, _)| self.is_char_like(el)));
                if self.is_aggregate(ty) && !whole {
                    // [dcl.init.aggr]/16: brace elision.
                    let l = self.ast.exprs.get(e).loc();
                    self.agg(ty, it, l, l)
                } else {
                    let x = it.next().expect("item");
                    self.build_init(ty, x)
                }
            }
            _ => panic!("unexpected designator"),
        }
    }

    // ---- statements ---------------------------------------------------------

    fn compound(&mut self) -> StmtId {
        let lb = self.expect("{");
        self.scopes.push(HashMap::new());
        let mut v = Vec::new();
        while !self.is("}") {
            v.push(self.stmt());
        }
        let rb = self.expect("}");
        self.scopes.pop();
        self.ast.builder().compound(&v, lb, rb)
    }

    fn decl_stmt(&mut self) -> StmtId {
        let l = self.loc();
        let ds = self.declaration(false);
        self.ast.builder().decl_stmt(&ds, l)
    }

    fn cond(&mut self) -> ExprId {
        self.expect("(");
        let e = self.expr();
        self.expect(")");
        self.to_bool(e)
    }

    fn stmt(&mut self) -> StmtId {
        let l = self.loc();
        if self.is("{") {
            return self.compound();
        }
        if self.eat(";") {
            return self.ast.builder().null_stmt(l);
        }
        if self.is("static_assert") {
            self.static_assert();
            return self.ast.builder().null_stmt(l);
        }
        let Tok::Ident(w) = self.peek().clone() else {
            return self.expr_stmt();
        };
        match w.as_str() {
            "if" => {
                self.next();
                let c = self.cond();
                let then = self.scoped_stmt();
                let els = if self.eat("else") { Some(self.scoped_stmt()) } else { None };
                let data =
                    IfData { init: None, cond_var: None, cond: Some(c), then: Some(then), els };
                self.ast.builder().if_stmt(IfKind::Ordinary, data, l)
            }
            "while" => {
                self.next();
                let c = self.cond();
                let body = self.scoped_stmt();
                let data = LoopData { cond: Some(c), body: Some(body), ..LoopData::default() };
                self.ast.builder().while_stmt(data, l)
            }
            "do" => {
                self.next();
                let body = self.scoped_stmt();
                self.expect("while");
                let c = self.cond();
                self.expect(";");
                self.ast.builder().do_stmt(body, c, l)
            }
            "for" => {
                self.next();
                self.expect("(");
                self.scopes.push(HashMap::new());
                let init = if self.eat(";") {
                    None
                } else if self.starts_type() {
                    Some(self.decl_stmt())
                } else {
                    Some(self.expr_stmt())
                };
                let cond = if self.is(";") {
                    None
                } else {
                    let e = self.expr();
                    Some(self.to_bool(e))
                };
                self.expect(";");
                let inc = if self.is(")") { None } else { Some(self.expr()) };
                self.expect(")");
                let body = self.scoped_stmt();
                self.scopes.pop();
                let data = LoopData { init, cond_var: None, cond, inc, body: Some(body) };
                self.ast.builder().for_stmt(data, l)
            }
            "switch" => {
                self.next();
                self.expect("(");
                let e = self.expr();
                self.expect(")");
                let e = self.rvalue(e);
                let e = self.promote(e);
                let t = self.ety(e);
                self.switches.push((t, Vec::new()));
                let body = self.scoped_stmt();
                let (_, cases) = self.switches.pop().expect("switch");
                let cases = self.ast.lists.push(&cases);
                let data = SwitchData { init: None, cond_var: None, cond: e, body, cases };
                self.ast.builder().switch_stmt(data, l)
            }
            "case" => {
                self.next();
                let lhs = self.case_value();
                let rhs = if self.eat("...") { Some(self.case_value()) } else { None };
                self.expect(":");
                let sub = self.stmt();
                let s = self.ast.builder().case_stmt(CaseData { lhs, rhs, sub }, l);
                self.switches.last_mut().expect("case outside switch").1.push(s);
                s
            }
            "default" => {
                self.next();
                self.expect(":");
                let sub = self.stmt();
                let s = self.ast.builder().default_stmt(sub, l);
                self.switches.last_mut().expect("default outside switch").1.push(s);
                s
            }
            "break" => {
                self.next();
                self.expect(";");
                self.ast.builder().break_stmt(l)
            }
            "continue" => {
                self.next();
                self.expect(";");
                self.ast.builder().continue_stmt(l)
            }
            "return" => {
                self.next();
                let v = if self.eat(";") {
                    None
                } else {
                    let i = if self.is("{") { self.init_clause() } else { Init::E(self.expr()) };
                    self.expect(";");
                    let r = self.ret;
                    Some(self.build_init(r, i))
                };
                self.ast.builder().return_stmt(v, None, l)
            }
            "goto" => panic!("goto: kiln_ast has no GotoStmt builder"),
            _ if self.starts_type() => self.decl_stmt(),
            _ => self.expr_stmt(),
        }
    }

    fn scoped_stmt(&mut self) -> StmtId {
        self.scopes.push(HashMap::new());
        let s = self.stmt();
        self.scopes.pop();
        s
    }

    fn case_value(&mut self) -> ExprId {
        let e = self.cond_expr();
        let e = self.rvalue(e);
        let t = self.switches.last().expect("switch").0;
        let e = self.convert(e, t);
        let v = self.fold(e).expect("case label is not a constant expression");
        self.ast.builder().constant_expr(e, Some(v))
    }

    fn expr_stmt(&mut self) -> StmtId {
        let e = self.expr();
        self.expect(";");
        self.ast.builder().expr_stmt(e)
    }

    // ---- constant folding (parse-time) ----------------------------------------

    fn fold(&mut self, e: ExprId) -> Option<ConstValueId> {
        let mut b = Bridge::new(&mut self.ast, self.target);
        self.fold.try_fold(&mut b, e)
    }

    fn fold_int(&mut self, e: ExprId) -> i128 {
        let v = self.fold(e).expect("not an integral constant expression");
        match self.ast.consts.scalar(v) {
            Some(Scalar::Int { bits, signed: true, .. }) => bits.to_i128().expect("i128"),
            Some(Scalar::Int { bits, .. }) => bits.to_u128().expect("u128") as i128,
            Some(Scalar::Bool { value, .. }) => i128::from(value),
            s => panic!("not an integer constant: {s:?}"),
        }
    }

    // ---- conversions ----------------------------------------------------------

    fn icast(&mut self, ck: CastKind, e: ExprId, ty: QualType) -> ExprId {
        self.ast.builder().implicit_cast(ck, e, ty, ValueCategory::PRValue)
    }

    /// [conv.lval], [conv.array], [conv.func].
    fn rvalue(&mut self, e: ExprId) -> ExprId {
        if !self.vk(e).is_glvalue() {
            return e;
        }
        let t = self.ety(e);
        if let Some((elem, _)) = self.array_of(t) {
            let p = self.ast.types.pointer_to(elem);
            return self.icast(CastKind::ArrayToPointerDecay, e, p);
        }
        if matches!(self.view(t), TypeView::Function(_)) {
            let p = self.ast.types.pointer_to(t);
            return self.icast(CastKind::FunctionToPointerDecay, e, p);
        }
        let u = self.unqual(t);
        self.icast(CastKind::LValueToRValue, e, u)
    }

    fn bitfield_width(&self, e: ExprId) -> Option<u32> {
        let mut e = e;
        loop {
            let n = self.ast.exprs.get(e);
            match n.kind() {
                ExprKind::ImplicitCastExpr | ExprKind::ParenExpr => e = self.ast.exprs.operand(e),
                ExprKind::MemberExpr if n.object_kind() == ObjectKind::BitField => {
                    return self.bitfields.get(&self.ast.exprs.decl(e)).copied();
                }
                _ => return None,
            }
        }
    }

    fn promoted(&self, e: ExprId) -> Option<QualType> {
        let t = self.canon(self.ety(e));
        if let Some(w) = self.bitfield_width(e)
            && self.is_int(t)
        {
            let (_, s, _) = self.int_info(t)?;
            if w < 32 {
                return Some(QualType::INT);
            }
            if w == 32 && !s {
                return Some(QualType::UINT);
            }
        }
        if let TypeView::Enum(d) = self.view(t) {
            let (_, p, scoped) = self.enums[&d];
            return (!scoped).then_some(p);
        }
        match self.builtin(t)? {
            B::Bool
            | B::Char
            | B::SChar
            | B::UChar
            | B::Char8
            | B::Short
            | B::UShort
            | B::Char16 => Some(QualType::INT),
            B::Char32 => Some(QualType::UINT),
            B::WChar => Some(QualType::INT),
            _ => None,
        }
    }

    /// [conv.prom] on a prvalue.
    fn promote(&mut self, e: ExprId) -> ExprId {
        let e = self.rvalue(e);
        match self.promoted(e) {
            Some(t) => self.convert(e, t),
            None => e,
        }
    }

    fn is_null_constant(&self, e: ExprId) -> bool {
        let mut e = e;
        while self.ast.exprs.get(e).kind() == ExprKind::ParenExpr {
            e = self.ast.exprs.operand(e);
        }
        let n = self.ast.exprs.get(e);
        (n.kind() == ExprKind::IntegerLiteral && self.ast.exprs.int_value(e) == Some(0))
            || self.is_nullptr_t(n.ty())
    }

    fn cast_kind(&mut self, e: ExprId, to: QualType, explicit: bool) -> CastKind {
        let from = self.unqual(self.ety(e));
        let to = self.unqual(to);
        if from == to {
            return CastKind::NoOp;
        }
        if self.builtin(to) == Some(B::Void) {
            return CastKind::ToVoid;
        }
        let fi = self.is_int(from);
        let ff = self.is_float(from);
        let fp = self.is_ptr(from) || self.is_nullptr_t(from);
        if self.is_bool(to) {
            return if fi {
                CastKind::IntegralToBoolean
            } else if ff {
                CastKind::FloatingToBoolean
            } else {
                CastKind::PointerToBoolean
            };
        }
        if self.is_int(to) {
            return if fi {
                CastKind::IntegralCast
            } else if ff {
                CastKind::FloatingToIntegral
            } else {
                assert!(explicit, "pointer to integer needs a cast");
                CastKind::PointerToIntegral
            };
        }
        if self.is_float(to) {
            return if fi { CastKind::IntegralToFloating } else { CastKind::FloatingCast };
        }
        if let Some(tp) = self.pointee(to) {
            if self.is_nullptr_t(from) || (fi && self.is_null_constant(e)) {
                return CastKind::NullToPointer;
            }
            if fi {
                assert!(explicit, "integer to pointer needs a cast");
                return CastKind::IntegralToPointer;
            }
            let fpe = self.pointee(from).expect("pointer");
            let (a, b) = (self.unqual(fpe), self.unqual(tp));
            if a == b {
                return CastKind::NoOp;
            }
            assert!(explicit || self.builtin(b) == Some(B::Void), "incompatible pointer types");
            return CastKind::BitCast;
        }
        let _ = fp;
        panic!("no conversion")
    }

    /// Implicit conversion of a prvalue to `to` ([conv]).
    fn convert(&mut self, e: ExprId, to: QualType) -> ExprId {
        let e = self.rvalue(e);
        let to_u = self.unqual(to);
        if self.unqual(self.ety(e)) == to_u {
            return e;
        }
        if self.is_record(to_u).is_some() {
            return e;
        }
        let ck = self.cast_kind(e, to_u, false);
        self.icast(ck, e, to_u)
    }

    fn to_bool(&mut self, e: ExprId) -> ExprId {
        let e = self.rvalue(e);
        self.convert(e, QualType::BOOL)
    }

    /// [expr.arith.conv]: the common type of two arithmetic prvalues.
    fn common(&mut self, a: ExprId, b: ExprId) -> QualType {
        let ta = self.unqual(self.ety(a));
        let tb = self.unqual(self.ety(b));
        match (self.float_rank(ta), self.float_rank(tb)) {
            (Some(x), Some(y)) => return if x >= y { ta } else { tb },
            (Some(_), None) => return ta,
            (None, Some(_)) => return tb,
            _ => {}
        }
        if ta == tb {
            return ta;
        }
        let (wa, sa, ra) = self.int_info(ta).expect("arithmetic");
        let (wb, sb, rb) = self.int_info(tb).expect("arithmetic");
        if sa == sb {
            return if ra >= rb { ta } else { tb };
        }
        let (s, ws, rs, u, ru) = if sa { (ta, wa, ra, tb, rb) } else { (tb, wb, rb, ta, ra) };
        let wu = if sa { wb } else { wa };
        if ru >= rs {
            u
        } else if ws > wu {
            s
        } else {
            Self::unsigned_of(s)
        }
    }

    fn arith(&mut self, a: ExprId, b: ExprId) -> (ExprId, ExprId, QualType) {
        let a = self.promote(a);
        let b = self.promote(b);
        let ta = self.unqual(self.ety(a));
        if self.is_scoped_enum(ta) {
            return (a, b, ta);
        }
        let t = self.common(a, b);
        let a = self.convert(a, t);
        let b = self.convert(b, t);
        (a, b, t)
    }

    // ---- expressions ----------------------------------------------------------

    pub fn expr(&mut self) -> ExprId {
        let mut e = self.assign_expr();
        while self.is(",") {
            let l = self.expect(",");
            let r = self.assign_expr();
            let t = self.ety(r);
            e = self.ast.builder().binary(BinaryOp::Comma, e, r, t, l);
        }
        e
    }

    fn assign_op(&self) -> Option<BinaryOp> {
        let Tok::Punct(p) = self.peek() else { return None };
        Some(match *p {
            "=" => BinaryOp::Assign,
            "*=" => BinaryOp::MulAssign,
            "/=" => BinaryOp::DivAssign,
            "%=" => BinaryOp::RemAssign,
            "+=" => BinaryOp::AddAssign,
            "-=" => BinaryOp::SubAssign,
            "<<=" => BinaryOp::ShlAssign,
            ">>=" => BinaryOp::ShrAssign,
            "&=" => BinaryOp::AndAssign,
            "^=" => BinaryOp::XorAssign,
            "|=" => BinaryOp::OrAssign,
            _ => return None,
        })
    }

    fn assign_expr(&mut self) -> ExprId {
        let lhs = self.cond_expr();
        let Some(op) = self.assign_op() else { return lhs };
        let l = self.loc();
        self.next();
        let rhs = self.assign_expr();
        let lt = self.ety(lhs);
        assert!(self.vk(lhs).is_glvalue(), "assignment to an rvalue at {l:?}");
        if op == BinaryOp::Assign {
            let r = if self.is_record(lt).is_some() { rhs } else { self.convert(rhs, lt) };
            return self.ast.builder().binary(op, lhs, r, lt, l);
        }
        // [expr.assign]/6: E1 op= E2 is E1 = E1 op E2 with E1 evaluated once.
        let lu = self.unqual(lt);
        let (r, comp) = if self.is_ptr(lu) {
            (self.rvalue(rhs), lu)
        } else if matches!(op, BinaryOp::ShlAssign | BinaryOp::ShrAssign) {
            let p = self.promoted(lhs).unwrap_or(lu);
            (self.promote(rhs), p)
        } else {
            let lp = self.promoted(lhs).unwrap_or(lu);
            let r = self.promote(rhs);
            let tmp = self.ast.builder().implicit_value_init(lp);
            let t = self.common(tmp, r);
            (self.convert(r, t), t)
        };
        self.ast.builder().compound_assign(op, lhs, r, lt, comp, comp, l)
    }

    fn cond_expr(&mut self) -> ExprId {
        let c = self.binary(1);
        if !self.is("?") {
            return c;
        }
        let l = self.expect("?");
        let c = self.to_bool(c);
        let t = self.expr();
        self.expect(":");
        let f = self.assign_expr();
        let (tt, ft) = (self.ety(t), self.ety(f));
        let (tv, fv) = (self.vk(t), self.vk(f));
        if tv.is_glvalue() && fv.is_glvalue() && self.canon(tt) == self.canon(ft) {
            return self.ast.builder().conditional(c, t, f, tt, l);
        }
        let (t, f, ty) = if self.is_record(tt).is_some() {
            let u = self.unqual(tt);
            let t = self.rvalue(t);
            let f = self.rvalue(f);
            (t, f, u)
        } else if {
            let (a, b) = (self.unqual(tt), self.unqual(ft));
            self.is_ptr(a) || self.is_ptr(b)
        } {
            let t = self.rvalue(t);
            let f = self.rvalue(f);
            let ty = if self.is_null_constant(t) {
                self.unqual(self.ety(f))
            } else {
                self.unqual(self.ety(t))
            };
            (self.convert(t, ty), self.convert(f, ty), ty)
        } else if self.builtin(tt) == Some(B::Void) {
            (t, f, QualType::VOID)
        } else {
            self.arith(t, f)
        };
        self.ast.builder().conditional(c, t, f, ty, l)
    }

    fn bin_op(&self) -> Option<(BinaryOp, u32)> {
        let Tok::Punct(p) = self.peek() else { return None };
        Some(match *p {
            "||" => (BinaryOp::LOr, 1),
            "&&" => (BinaryOp::LAnd, 2),
            "|" => (BinaryOp::Or, 3),
            "^" => (BinaryOp::Xor, 4),
            "&" => (BinaryOp::And, 5),
            "==" => (BinaryOp::EQ, 6),
            "!=" => (BinaryOp::NE, 6),
            "<" => (BinaryOp::LT, 7),
            ">" => (BinaryOp::GT, 7),
            "<=" => (BinaryOp::LE, 7),
            ">=" => (BinaryOp::GE, 7),
            "<<" => (BinaryOp::Shl, 8),
            ">>" => (BinaryOp::Shr, 8),
            "+" => (BinaryOp::Add, 9),
            "-" => (BinaryOp::Sub, 9),
            "*" => (BinaryOp::Mul, 10),
            "/" => (BinaryOp::Div, 10),
            "%" => (BinaryOp::Rem, 10),
            _ => return None,
        })
    }

    fn binary(&mut self, min: u32) -> ExprId {
        let mut lhs = self.unary();
        while let Some((op, prec)) = self.bin_op() {
            if prec < min {
                break;
            }
            let l = self.loc();
            self.next();
            let rhs = self.binary(prec + 1);
            lhs = self.make_binary(op, lhs, rhs, l);
        }
        lhs
    }

    fn make_binary(&mut self, op: BinaryOp, a: ExprId, b: ExprId, l: Loc) -> ExprId {
        use BinaryOp::*;
        match op {
            LAnd | LOr => {
                let a = self.to_bool(a);
                let b = self.to_bool(b);
                self.ast.builder().binary(op, a, b, QualType::BOOL, l)
            }
            Shl | Shr => {
                let a = self.promote(a);
                let b = self.promote(b);
                let t = self.ety(a);
                self.ast.builder().binary(op, a, b, t, l)
            }
            Add | Sub => {
                let a = self.rvalue(a);
                let b = self.rvalue(b);
                let (ta, tb) = (self.unqual(self.ety(a)), self.unqual(self.ety(b)));
                match (self.is_ptr(ta), self.is_ptr(tb)) {
                    (true, true) => self.ast.builder().binary(op, a, b, QualType::LONG, l),
                    (true, false) => self.ast.builder().binary(op, a, b, ta, l),
                    (false, true) => self.ast.builder().binary(op, a, b, tb, l),
                    _ => {
                        let (a, b, t) = self.arith(a, b);
                        self.ast.builder().binary(op, a, b, t, l)
                    }
                }
            }
            EQ | NE | LT | GT | LE | GE => {
                let a = self.rvalue(a);
                let b = self.rvalue(b);
                let (ta, tb) = (self.unqual(self.ety(a)), self.unqual(self.ety(b)));
                let pa = self.is_ptr(ta) || self.is_nullptr_t(ta);
                let pb = self.is_ptr(tb) || self.is_nullptr_t(tb);
                let (a, b) = if pa || pb {
                    let t = if self.is_null_constant(a) && pb { tb } else { ta };
                    let t = if self.pointee(t).is_some_and(|p| self.builtin(p) == Some(B::Void)) {
                        t
                    } else if self.pointee(tb).is_some_and(|p| self.builtin(p) == Some(B::Void)) {
                        tb
                    } else {
                        t
                    };
                    (self.convert(a, t), self.convert(b, t))
                } else {
                    let (a, b, _) = self.arith(a, b);
                    (a, b)
                };
                self.ast.builder().binary(op, a, b, QualType::BOOL, l)
            }
            _ => {
                let (a, b, t) = self.arith(a, b);
                self.ast.builder().binary(op, a, b, t, l)
            }
        }
    }

    fn unary(&mut self) -> ExprId {
        let l = self.loc();
        let Tok::Punct(p) = self.peek().clone() else {
            if self.is("sizeof") || self.is("alignof") || self.is("_Alignof") {
                return self.trait_expr();
            }
            return self.postfix();
        };
        match p {
            "++" | "--" => {
                self.next();
                let e = self.unary();
                let op = if p == "++" { UnaryOp::PreInc } else { UnaryOp::PreDec };
                let t = self.ety(e);
                self.ast.builder().unary(op, e, t, l, ExprFlags::NONE)
            }
            "&" => {
                self.next();
                let e = self.cast_expr();
                let t = self.ety(e);
                let pt = self.ast.types.pointer_to(t);
                self.ast.builder().unary(UnaryOp::AddrOf, e, pt, l, ExprFlags::NONE)
            }
            "*" => {
                self.next();
                let e = self.cast_expr();
                let e = self.rvalue(e);
                let t = self.pointee(self.ety(e)).expect("dereference of a non-pointer");
                self.ast.builder().unary(UnaryOp::Deref, e, t, l, ExprFlags::NONE)
            }
            "+" | "-" | "~" => {
                self.next();
                let e = self.cast_expr();
                let e = self.promote(e);
                let t = self.ety(e);
                let op = match p {
                    "+" => UnaryOp::Plus,
                    "-" => UnaryOp::Minus,
                    _ => UnaryOp::Not,
                };
                self.ast.builder().unary(op, e, t, l, ExprFlags::NONE)
            }
            "!" => {
                self.next();
                let e = self.cast_expr();
                let e = self.to_bool(e);
                self.ast.builder().unary(UnaryOp::LNot, e, QualType::BOOL, l, ExprFlags::NONE)
            }
            "(" => self.cast_expr(),
            _ => self.postfix(),
        }
    }

    fn cast_expr(&mut self) -> ExprId {
        if self.is("(") {
            let save = self.pos;
            self.next();
            if self.starts_type() {
                let l = self.toks[save].at;
                let t = self.type_name();
                self.expect(")");
                if self.is("{") {
                    panic!("compound literals are not supported");
                }
                let e = self.cast_expr();
                return self.explicit_cast(ExprKind::CStyleCastExpr, e, t, loc_of(l));
            }
            self.pos = save;
        }
        self.unary()
    }

    fn explicit_cast(&mut self, kind: ExprKind, e: ExprId, t: QualType, l: Loc) -> ExprId {
        let tu = self.unqual(t);
        let e = if self.builtin(tu) == Some(B::Void) { e } else { self.rvalue(e) };
        let ck = self.cast_kind(e, tu, true);
        self.ast.builder().cast(kind, ck, e, tu, ValueCategory::PRValue, None, l, ExprFlags::NONE)
    }

    fn trait_expr(&mut self) -> ExprId {
        let l = self.loc();
        let (w, _) = self.ident();
        let kind = if w == "sizeof" { TraitKind::SizeOf } else { TraitKind::AlignOf };
        if self.is("(") {
            let save = self.pos;
            self.next();
            if self.starts_type() {
                let t = self.type_name();
                let r = self.expect(")");
                return self.ast.builder().trait_of_type(kind, t, QualType::ULONG, l, r);
            }
            self.pos = save;
        }
        let e = self.unary();
        self.ast.builder().trait_of_expr(kind, e, QualType::ULONG, l)
    }

    fn postfix(&mut self) -> ExprId {
        let mut e = self.primary();
        loop {
            let l = self.loc();
            if self.eat("[") {
                let i = self.expr();
                self.expect("]");
                let b = self.rvalue(e);
                let i = self.rvalue(i);
                let t = self.pointee(self.ety(b)).expect("subscript of a non-pointer");
                let bl = self.ast.exprs.get(b).loc();
                e = self.ast.builder().subscript(b, i, t, bl);
            } else if self.eat("(") {
                e = self.call(e, l);
            } else if self.is(".") || self.is("->") {
                let arrow = self.ident_or_punct() == "->";
                let (n, ml) = self.ident();
                let (base, rt) = if arrow {
                    let b = self.rvalue(e);
                    (b, self.pointee(self.ety(b)).expect("-> on a non-pointer"))
                } else {
                    assert!(self.vk(e).is_glvalue(), "member of a prvalue is not supported");
                    (e, self.ety(e))
                };
                let r = self.is_record(rt).expect("member of a non-class");
                let f = self.fields[&r]
                    .iter()
                    .find(|f| f.name.as_deref() == Some(n.as_str()))
                    .unwrap_or_else(|| panic!("no member {n}"))
                    .clone();
                let ft = if self.is_const(rt) { self.add_const(f.ty) } else { f.ty };
                e = self.ast.builder().member(base, f.decl, ft, arrow, ml, None);
            } else if self.is("++") || self.is("--") {
                let op =
                    if self.ident_or_punct() == "++" { UnaryOp::PostInc } else { UnaryOp::PostDec };
                let t = self.unqual(self.ety(e));
                e = self.ast.builder().unary(op, e, t, l, ExprFlags::NONE);
            } else {
                return e;
            }
        }
    }

    fn ident_or_punct(&mut self) -> String {
        match self.next().tok {
            Tok::Punct(p) => p.to_owned(),
            Tok::Ident(s) => s,
            t => panic!("unexpected {t:?}"),
        }
    }

    fn call(&mut self, callee: ExprId, lparen: Loc) -> ExprId {
        let mut args = Vec::new();
        while !self.is(")") {
            args.push(self.assign_expr());
            if !self.eat(",") {
                break;
            }
        }
        let rp = self.expect(")");
        let c = self.rvalue(callee);
        let ft = self.pointee(self.ety(c)).expect("call of a non-function");
        let TypeView::Function(proto) = self.view(ft) else { panic!("call of a non-function") };
        let ret = proto.ret();
        let pts = self.ast.types.params(&proto).to_vec();
        assert_eq!(pts.len(), args.len(), "argument count at {lparen:?}");
        let args: Vec<ExprId> = args
            .into_iter()
            .zip(pts)
            .map(
                |(a, t)| {
                    if self.is_record(t).is_some() { self.rvalue(a) } else { self.convert(a, t) }
                },
            )
            .collect();
        let l = self.ast.exprs.get(callee).loc();
        let ret = self.unqual(ret);
        self.ast.builder().call(c, &args, ret, ValueCategory::PRValue, l, rp, ExprFlags::NONE)
    }

    fn primary(&mut self) -> ExprId {
        let l = self.loc();
        let t = self.next();
        match t.tok {
            Tok::Num(s) => self.number(&s, l),
            Tok::Char(s) => self.char_lit(&s, l),
            Tok::Str(s) => {
                let mut parts = vec![s];
                while let Tok::Str(s) = self.peek().clone() {
                    self.next();
                    parts.push(s);
                }
                self.string_lit(&parts, l)
            }
            Tok::Punct("(") => {
                let e = self.expr();
                let r = self.expect(")");
                self.ast.builder().paren(e, l, r)
            }
            Tok::Ident(w) => match w.as_str() {
                "true" | "false" => self.ast.builder().bool_literal(QualType::BOOL, w == "true", l),
                "nullptr" => self.ast.builder().nullptr_literal(QualType::NULLPTR, l),
                "static_cast" | "reinterpret_cast" | "const_cast" => {
                    self.expect("<");
                    let ty = self.type_name();
                    self.expect(">");
                    self.expect("(");
                    let e = self.expr();
                    self.expect(")");
                    let kind = match w.as_str() {
                        "static_cast" => ExprKind::CXXStaticCastExpr,
                        "reinterpret_cast" => ExprKind::CXXReinterpretCastExpr,
                        _ => ExprKind::CXXConstCastExpr,
                    };
                    self.explicit_cast(kind, e, ty, l)
                }
                _ => {
                    let mut name = w.clone();
                    if self.is("::") {
                        self.next();
                        let (m, _) = self.ident();
                        name = format!("{w}::{m}");
                        let d = self.names[&name];
                        let ty = self.tags[&w];
                        return self.ast.builder().decl_ref(
                            d,
                            ty,
                            l,
                            None,
                            NonOdrUse::None,
                            ExprFlags::NONE,
                        );
                    }
                    let (d, ty) = match self.lookup(&name) {
                        Some(Sym::Var(d, t) | Sym::Func(d, t) | Sym::Enumerator(d, t)) => (d, t),
                        _ => panic!("undeclared identifier {name} at {l:?}"),
                    };
                    self.ast.builder().decl_ref(d, ty, l, None, NonOdrUse::None, ExprFlags::NONE)
                }
            },
            t => panic!("expected an expression at {l:?}, found {t:?}"),
        }
    }

    fn number(&mut self, s: &str, l: Loc) -> ExprId {
        let lower = s.to_ascii_lowercase();
        let is_hex = lower.starts_with("0x");
        let is_float = !is_hex && (lower.contains('.') || lower.contains('e'))
            || (is_hex && lower.contains('p'));
        if is_float {
            let (body, ty, kind) = if let Some(b) = lower.strip_suffix('f') {
                (b, QualType::FLOAT, FloatKind::Single)
            } else if let Some(b) = lower.strip_suffix('l') {
                (b, QualType::LONGDOUBLE, FloatKind::X87Extended)
            } else {
                (lower.as_str(), QualType::DOUBLE, FloatKind::Double)
            };
            let bits = match kind {
                FloatKind::Single => u128::from(body.parse::<f32>().expect("float").to_bits()),
                FloatKind::Double => u128::from(body.parse::<f64>().expect("double").to_bits()),
                _ => f64_to_x87(body.parse::<f64>().expect("long double")),
            };
            let cv = self.ast.consts.push_scalar(Scalar::Float { ty, kind, bits }).expect("float");
            return self.ast.builder().float_literal(ty, cv, l);
        }
        let digits = lower.trim_end_matches(['u', 'l']);
        let suffix = &lower[digits.len()..];
        let (radix, v) = if is_hex {
            (Radix::Hex, u128::from_str_radix(&digits[2..], 16))
        } else if let Some(b) = digits.strip_prefix("0b") {
            (Radix::Binary, u128::from_str_radix(b, 2))
        } else if digits.len() > 1 && digits.starts_with('0') {
            (Radix::Octal, u128::from_str_radix(&digits[1..], 8))
        } else {
            (Radix::Decimal, digits.parse::<u128>())
        };
        let v = v.expect("integer literal");
        let unsigned = suffix.contains('u');
        let longs = suffix.matches('l').count();
        let decimal = radix == Radix::Decimal;
        // [lex.icon] Table 7: the first type that can represent the value.
        let cands: &[QualType] = match (unsigned, longs) {
            (false, 0) if decimal => &[QualType::INT, QualType::LONG, QualType::LONGLONG],
            (false, 0) => &[
                QualType::INT,
                QualType::UINT,
                QualType::LONG,
                QualType::ULONG,
                QualType::LONGLONG,
                QualType::ULONGLONG,
            ],
            (false, 1) if decimal => &[QualType::LONG, QualType::LONGLONG],
            (false, 1) => {
                &[QualType::LONG, QualType::ULONG, QualType::LONGLONG, QualType::ULONGLONG]
            }
            (false, _) if decimal => &[QualType::LONGLONG],
            (false, _) => &[QualType::LONGLONG, QualType::ULONGLONG],
            (true, 0) => &[QualType::UINT, QualType::ULONG, QualType::ULONGLONG],
            (true, 1) => &[QualType::ULONG, QualType::ULONGLONG],
            (true, _) => &[QualType::ULONGLONG],
        };
        let ty = cands
            .iter()
            .copied()
            .find(|&t| {
                let (w, s, _) = self.int_info(t).expect("int");
                v < 1u128 << (w - u32::from(s))
            })
            .unwrap_or_else(|| panic!("integer literal {s} is too large"));
        self.ast.builder().int_literal(ty, u64::try_from(v).expect("u64"), radix, l)
    }

    fn char_lit(&mut self, s: &str, l: Loc) -> ExprId {
        let (prefix, body) = s.split_at(s.find('\'').expect("quote"));
        let body = &body[1..body.len() - 1];
        let cps = decode(body);
        assert_eq!(cps.len(), 1, "multi-character literal");
        let (ty, kind) = match prefix {
            "u8" => (QualType::CHAR8, CharKind::Utf8),
            "u" => (QualType::CHAR16, CharKind::Utf16),
            "U" => (QualType::CHAR32, CharKind::Utf32),
            "L" => (QualType::WCHAR, CharKind::Wide),
            _ => (QualType::CHAR, CharKind::Ascii),
        };
        self.ast.builder().char_literal(ty, kind, cps[0], l)
    }

    fn string_lit(&mut self, parts: &[String], l: Loc) -> ExprId {
        let mut prefix = "";
        let mut cps = Vec::new();
        for p in parts {
            let q = p.find('"').expect("quote");
            if q > 0 {
                prefix = &p[..q];
            }
            cps.extend(decode(&p[q + 1..p.len() - 1]));
        }
        let (enc, kind, elem, w) = match prefix {
            "u8" => (StringEncoding::Utf8, StringKind::Utf8, QualType::CHAR8, 1),
            "u" => (StringEncoding::Utf16, StringKind::Utf16, QualType::CHAR16, 2),
            "U" => (StringEncoding::Utf32, StringKind::Utf32, QualType::CHAR32, 4),
            "L" => (StringEncoding::Wide32, StringKind::Wide, QualType::WCHAR, 4),
            _ => (StringEncoding::Ordinary, StringKind::Ordinary, QualType::CHAR, 1),
        };
        let mut bytes = Vec::new();
        for &c in &cps {
            bytes.extend_from_slice(&c.to_le_bytes()[..w]);
        }
        let id = self.ast.strings.intern(enc, &bytes);
        let ce = self.add_const(elem);
        let ty = self.ast.types.constant_array(ce, cps.len() as u64 + 1, None);
        self.ast.builder().string_literal(ty, kind, id, l)
    }
}

/// The x87 80-bit encoding of a double (exact: every double is
/// representable).
pub fn f64_to_x87(d: f64) -> u128 {
    let b = d.to_bits();
    let sign = u128::from(b >> 63) << 79;
    let exp = ((b >> 52) & 0x7ff) as i64;
    let frac = b & ((1 << 52) - 1);
    if exp == 0 && frac == 0 {
        return sign;
    }
    if exp == 0x7ff {
        let m = if frac == 0 {
            1u128 << 63
        } else {
            (1u128 << 63) | (1u128 << 62) | (u128::from(frac) << 11)
        };
        return sign | (0x7fffu128 << 64) | m;
    }
    let (e, m) = if exp == 0 {
        // Subnormal double: normalize.
        let lz = frac.leading_zeros() - 11;
        (1 - 1023 - i64::from(lz) + 16383, u128::from(frac << (lz + 11)))
    } else {
        (exp - 1023 + 16383, (1u128 << 63) | (u128::from(frac) << 11))
    };
    sign | ((e as u128) << 64) | m
}
