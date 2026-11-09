//! Kind-2 module files, part 2a: the declaration and type image
//! (spec/modules.md §11.11 "Part 2a").
//!
//! The writer serialises the declarations of a module interface unit's
//! purview and the types they use, with every id rewritten to an index in
//! the image (one index space per arena: names, types, declarations). An
//! importer that is given the image builds the unit from it through the
//! [`kiln_ast::AstBuilder`] instead of replaying the unit's source: one
//! builder call per record, no lexing, parsing or semantic analysis.
//!
//! Coverage. Only what can be rebuilt exactly is written: namespaces,
//! export declarations, non-inline non-constexpr functions (the body is
//! not needed by an importer, which neither emits nor evaluates it: only
//! its existence and braces are kept), variables without initializers,
//! enumerations and their enumerators (with their values), and typedefs;
//! types built from builtins, pointers, references, arrays, function
//! types, enumerations and typedefs. Anything else (a class, a template,
//! an `inline` or `constexpr` function, an initializer, an attribute, a
//! global module fragment, an import, a declaration outside the unit's
//! main file, ...) makes the writer emit no image, and the importer
//! replays the source as before (the fallback; correctness first).
//!
//! Reading is total: [`decode`] validates every index, offset and length
//! before anything is built; a malformed image is the module file's
//! corrupt-file error and builds nothing.

use kiln_ast::consts::Scalar;
use kiln_ast::decl::{
    Access, ConstexprKind, DeclFlags, DeclKind, EnumData, FnSpec, FnStorage, FriendKind,
    InitStyle, ModuleOwnership, ParamData, VarSpec, VarStorage,
};
use kiln_ast::modules::Owner;
use kiln_ast::names::DeclName;
use kiln_ast::stmt::StmtKind;
use kiln_ast::types::{BuiltinType, ExcSpec, FuncInfo, Quals, TagKeyword, TypeView};
use kiln_ast::{DeclHead, DeclId, QualType, TypeId};
use kiln_source::fx::FxHashMap;
use kiln_source::{FileId, Loc};

use crate::Sema;
use crate::modules::Directive;

mod body;

use body::{Bodies, BodyState, Root};

/// The image format's version (the first field); bumped on any change.
pub(crate) const IMAGE_VERSION: u32 = 2;

/// "None" in an index or offset field.
const NONE: u32 = u32::MAX;
/// The translation unit as a lexical context.
const LEX_TU: u32 = u32::MAX - 1;
/// Type references `1..=BUILTINS` are the builtins; `BUILTINS + 1 + i`
/// is the image's type `i`.
const BUILTINS: u32 = BuiltinType::ALL.len() as u32;

/// What the writer hands the driver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleImage {
    /// The [`kiln_pp::bmi::SECTION_IMAGE`] bytes; empty when there is no
    /// image.
    pub bytes: Vec<u8>,
    /// Declaration records in the image (parameters not counted).
    pub decls: usize,
    /// Why there is no image (the first unsupported construct).
    pub fallback: Option<&'static str>,
}

// ---------------------------------------------------------------------------
// The image model
// ---------------------------------------------------------------------------

/// A qualified type reference: builtin or image type, plus cv bits.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct QRef {
    ty: u32,
    quals: u8,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum TyRec {
    Pointer(QRef),
    LRef(QRef),
    RRef(QRef),
    CArray(QRef, u64),
    IArray(QRef),
    Paren(QRef),
    Elab(u8, QRef),
    Func {
        ret: QRef,
        params: Vec<QRef>,
        info: u16,
        exc: u8,
    },
    Enum(u32),
    Typedef(u32),
    /// cv on array sugar (`const Quad` for `using Quad = int[4]`): the
    /// base, qualified through `TypeContext::qualify`.
    QArray(u8, QRef),
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct ParamRec {
    /// Index into the parameter names, or [`NONE`].
    name: u32,
    loc: u32,
    ty: QRef,
    /// `ParamData.flags` (scope depth only).
    pflags: u16,
    /// `USED` / `REFERENCED`.
    flags: u8,
    /// The default argument's expression record, or [`NONE`].
    default: u32,
}

/// A function's body.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FnBody {
    None,
    /// The braces of a body the image does not carry (a non-inline
    /// function: importers only need to know it is defined).
    Braces(u32, u32),
    /// The body's compound statement record (inline, `constexpr` and
    /// `consteval` functions).
    Stmt(u32),
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum Payload {
    Namespace,
    Export,
    Function {
        ty: QRef,
        /// `FnSpec` bits (the subset [`fn_spec`] accepts).
        spec: u32,
        params: Vec<ParamRec>,
        body: FnBody,
    },
    Var {
        ty: QRef,
        /// `VarSpec` bits (the subset [`var_spec`] accepts).
        spec: u32,
        /// The initializer's expression record, or [`NONE`].
        init: u32,
        /// The evaluated value: [`NONE`] not evaluated (or a value the
        /// image does not carry), [`body::NOT_CONST`], or a constant.
        eval: u32,
    },
    /// A label (a local declaration only).
    Label,
    /// `scoped` is `EnumData` `SCOPED | SCOPED_STRUCT`; `def` is present
    /// for a definition, holding the computed (underlying, promotion)
    /// types of one without a fixed underlying type.
    Enum {
        scoped: u8,
        fixed: Option<(QRef, QRef)>,
        // Three states: a declaration only, a definition with a fixed
        // type, a definition with computed types (spec/modules.md §11.11
        // "Encoding", the enumeration payload).
        #[allow(clippy::option_option)]
        def: Option<Option<(QRef, QRef)>>,
    },
    Enumerator {
        ty: QRef,
        value: i128,
    },
    Typedef {
        alias: bool,
        underlying: QRef,
    },
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct DeclRec {
    /// `USED` / `REFERENCED`.
    flags: u8,
    /// Bit 0 purview, bit 1 exported.
    owner: u8,
    /// Index into the names, or [`NONE`].
    name: u32,
    /// Record index, or [`NONE`] for the translation unit.
    ctx: u32,
    /// Record index, [`LEX_TU`], or [`NONE`] when it is `ctx`.
    lexical: u32,
    loc: u32,
    /// Record index of the previous declaration, or [`NONE`].
    prev: u32,
    payload: Payload,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
struct Image {
    main: String,
    module: String,
    kw: u32,
    name_loc: u32,
    names: Vec<String>,
    pnames: Vec<String>,
    types: Vec<TyRec>,
    decls: Vec<DeclRec>,
    bodies: Bodies,
}

const K_NAMESPACE: u8 = 1;
const K_EXPORT: u8 = 2;
const K_FUNCTION: u8 = 3;
const K_VAR: u8 = 4;
const K_ENUM: u8 = 5;
const K_ENUMERATOR: u8 = 6;
const K_TYPEDEF: u8 = 7;
const K_ALIAS: u8 = 8;
const K_LABEL: u8 = 9;

impl Payload {
    fn code(&self) -> u8 {
        match self {
            Payload::Namespace => K_NAMESPACE,
            Payload::Export => K_EXPORT,
            Payload::Function { .. } => K_FUNCTION,
            Payload::Var { .. } => K_VAR,
            Payload::Enum { .. } => K_ENUM,
            Payload::Enumerator { .. } => K_ENUMERATOR,
            Payload::Typedef { alias: false, .. } => K_TYPEDEF,
            Payload::Typedef { alias: true, .. } => K_ALIAS,
            Payload::Label => K_LABEL,
        }
    }
}

const DECL_FLAGS: u8 = DeclFlags::USED.bits() | DeclFlags::REFERENCED.bits();

/// Function specifier bits carried: storage, inline (written and
/// implicit), `constexpr`/`consteval`, immediate-escalating, escalated,
/// `...`, inherited storage, deduced return type. Anything else (virtual,
/// deleted, defaulted, coroutine, contracts, ...) is not carried yet.
fn fn_spec(bits: u32) -> Option<FnSpec> {
    let storage = match bits & 3 {
        0 => FnStorage::None,
        1 => FnStorage::Static,
        2 => FnStorage::Extern,
        _ => return None,
    };
    let ck = match (bits >> 4) & 3 {
        0 => ConstexprKind::None,
        1 => ConstexprKind::Constexpr,
        2 => ConstexprKind::Consteval,
        _ => return None,
    };
    let bit = |b: u32| bits & (1 << b) != 0;
    let spec = FnSpec::new()
        .with_storage(storage)
        .with_inline(bit(2))
        .with_implicit_inline(bit(3))
        .with_constexpr_kind(ck)
        .with_immediate_escalating(bit(13))
        .with_escalated(bit(16))
        .with_variadic(bit(17))
        .with_storage_inherited(bit(22))
        .with_return_deduced(bit(24));
    (spec.bits() == bits).then_some(spec)
}

/// Variable specifier bits carried: storage (none, static, extern), the
/// initialization style, `constexpr`, `constinit`, `inline`, NRVO and
/// range-`for` variables. Thread-local storage is not carried yet.
fn var_spec(bits: u32) -> Option<VarSpec> {
    let storage = match bits & 7 {
        0 => VarStorage::None,
        1 => VarStorage::Static,
        2 => VarStorage::Extern,
        _ => return None,
    };
    let style = match (bits >> 5) & 3 {
        0 => InitStyle::C,
        1 => InitStyle::Call,
        2 => InitStyle::List,
        _ => InitStyle::ParenList,
    };
    let bit = |b: u32| bits & (1 << b) != 0;
    let spec = VarSpec::new()
        .with_storage(storage)
        .with_init_style(style)
        .with_constexpr(bit(7))
        .with_constinit(bit(8))
        .with_inline(bit(9))
        .with_nrvo(bit(10))
        .with_cxx_for_range(bit(12));
    (spec.bits() == bits).then_some(spec)
}

fn tag_keyword(k: u8) -> Option<TagKeyword> {
    Some(match k {
        0 => TagKeyword::None,
        1 => TagKeyword::Struct,
        2 => TagKeyword::Class,
        3 => TagKeyword::Union,
        4 => TagKeyword::Enum,
        5 => TagKeyword::Typename,
        6 => TagKeyword::Interface,
        _ => return None,
    })
}

fn exc_spec(e: u8) -> Option<ExcSpec> {
    Some(match e {
        0 => ExcSpec::None,
        1 => ExcSpec::BasicNoexcept,
        2 => ExcSpec::DynamicNone,
        3 => ExcSpec::Nothrow,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Encoding
// ---------------------------------------------------------------------------

struct Wr(Vec<u8>);

impl Wr {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn len(&mut self, n: usize) {
        self.u32(u32::try_from(n).unwrap_or(u32::MAX));
    }
    fn str(&mut self, s: &str) {
        self.len(s.len());
        self.0.extend_from_slice(s.as_bytes());
    }
    fn q(&mut self, q: QRef) {
        self.u32(q.ty);
        self.u8(q.quals);
    }
    fn pair(&mut self, p: Option<(QRef, QRef)>) {
        match p {
            None => self.u8(0),
            Some((a, b)) => {
                self.u8(1);
                self.q(a);
                self.q(b);
            }
        }
    }
}

fn encode_decl(w: &mut Wr, d: &DeclRec) {
    w.u8(d.payload.code());
    w.u8(d.flags);
    w.u8(d.owner);
    for v in [d.name, d.ctx, d.lexical, d.loc, d.prev] {
        w.u32(v);
    }
    match &d.payload {
        Payload::Namespace | Payload::Export => {}
        Payload::Function { ty, spec, params, body } => {
            w.q(*ty);
            w.u32(*spec);
            w.len(params.len());
            for p in params {
                w.u32(p.name);
                w.u32(p.loc);
                w.q(p.ty);
                w.u16(p.pflags);
                w.u8(p.flags);
                w.u32(p.default);
            }
            match body {
                FnBody::None => w.u8(0),
                FnBody::Braces(lb, rb) => {
                    w.u8(1);
                    w.u32(*lb);
                    w.u32(*rb);
                }
                FnBody::Stmt(s) => {
                    w.u8(2);
                    w.u32(*s);
                }
            }
        }
        Payload::Var { ty, spec, init, eval } => {
            w.q(*ty);
            w.u32(*spec);
            w.u32(*init);
            w.u32(*eval);
        }
        Payload::Label => {}
        Payload::Enum { scoped, fixed, def } => {
            w.u8(*scoped);
            w.pair(*fixed);
            match def {
                None => w.u8(0),
                Some(c) => {
                    w.u8(1);
                    w.pair(*c);
                }
            }
        }
        Payload::Enumerator { ty, value } => {
            w.q(*ty);
            w.0.extend_from_slice(&value.to_le_bytes());
        }
        Payload::Typedef { underlying, .. } => w.q(*underlying),
    }
}

fn encode(img: &Image) -> Vec<u8> {
    let mut w = Wr(Vec::new());
    w.u32(IMAGE_VERSION);
    w.str(&img.main);
    w.str(&img.module);
    w.u32(img.kw);
    w.u32(img.name_loc);
    for list in [&img.names, &img.pnames] {
        w.len(list.len());
        for s in list {
            w.str(s);
        }
    }
    w.len(img.types.len());
    for t in &img.types {
        match t {
            TyRec::Pointer(q) => (w.u8(1), w.q(*q)).1,
            TyRec::LRef(q) => (w.u8(2), w.q(*q)).1,
            TyRec::RRef(q) => (w.u8(3), w.q(*q)).1,
            TyRec::CArray(q, n) => {
                w.u8(4);
                w.q(*q);
                w.0.extend_from_slice(&n.to_le_bytes());
            }
            TyRec::IArray(q) => (w.u8(5), w.q(*q)).1,
            TyRec::Paren(q) => (w.u8(6), w.q(*q)).1,
            TyRec::Elab(k, q) => {
                w.u8(7);
                w.u8(*k);
                w.q(*q);
            }
            TyRec::Func { ret, params, info, exc } => {
                w.u8(8);
                w.q(*ret);
                w.len(params.len());
                for p in params {
                    w.q(*p);
                }
                w.u16(*info);
                w.u8(*exc);
            }
            TyRec::Enum(d) => (w.u8(9), w.u32(*d)).1,
            TyRec::Typedef(d) => (w.u8(10), w.u32(*d)).1,
            TyRec::QArray(c, q) => {
                w.u8(11);
                w.u8(*c);
                w.q(*q);
            }
        }
    }
    // Declarations: an offset table into the record blob, so a reader
    // can find record `i` without decoding the ones before it.
    let mut blob = Wr(Vec::new());
    let mut offsets = Vec::with_capacity(img.decls.len());
    for d in &img.decls {
        offsets.push(u32::try_from(blob.0.len()).unwrap_or(u32::MAX));
        encode_decl(&mut blob, d);
    }
    w.len(img.decls.len());
    for o in offsets {
        w.u32(o);
    }
    w.len(blob.0.len());
    w.0.extend_from_slice(&blob.0);
    // The top-level lookup table: (context, name, record), sorted.
    let mut lookup: Vec<(u32, u32, u32)> = img
        .decls
        .iter()
        .enumerate()
        .filter(|(_, d)| d.name != NONE && !body::is_local(img, d))
        .map(|(i, d)| (d.ctx, d.name, i as u32))
        .collect();
    lookup.sort_unstable();
    w.len(lookup.len());
    for (c, n, d) in lookup {
        w.u32(c);
        w.u32(n);
        w.u32(d);
    }
    body::encode_bodies(&mut w, &img.bodies);
    w.0
}

// ---------------------------------------------------------------------------
// Decoding (total)
// ---------------------------------------------------------------------------

/// A malformed image; the text is the corrupt-file diagnostic's reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ImageError(pub String);

type Res<T> = Result<T, ImageError>;

fn bad<T>(why: &str) -> Res<T> {
    Err(ImageError(format!("declaration image: {why}")))
}

struct Rd<'a>(&'a [u8]);

impl<'a> Rd<'a> {
    fn take(&mut self, n: usize) -> Res<&'a [u8]> {
        if n > self.0.len() {
            return bad("truncated");
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn u8(&mut self) -> Res<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Res<u16> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Res<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn u64(&mut self) -> Res<u64> {
        let mut a = [0u8; 8];
        a.copy_from_slice(self.take(8)?);
        Ok(u64::from_le_bytes(a))
    }
    fn i128(&mut self) -> Res<i128> {
        let mut a = [0u8; 16];
        a.copy_from_slice(self.take(16)?);
        Ok(i128::from_le_bytes(a))
    }
    /// A count whose elements take at least `min` bytes each, checked
    /// against the bytes left before anything is sized by it.
    fn count(&mut self, min: usize) -> Res<usize> {
        let n = self.u32()? as usize;
        if n.saturating_mul(min.max(1)) > self.0.len() {
            return bad("length exceeds the section");
        }
        Ok(n)
    }
    fn str(&mut self) -> Res<String> {
        let n = self.count(1)?;
        match std::str::from_utf8(self.take(n)?) {
            Ok(s) => Ok(s.to_owned()),
            Err(_) => bad("invalid UTF-8 in a name"),
        }
    }
    fn q(&mut self) -> Res<QRef> {
        let ty = self.u32()?;
        let quals = self.u8()?;
        if quals > 7 {
            return bad("bad qualifier bits");
        }
        Ok(QRef { ty, quals })
    }
    fn pair(&mut self) -> Res<Option<(QRef, QRef)>> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some((self.q()?, self.q()?))),
            _ => bad("bad flag byte"),
        }
    }
}

/// The smallest encoded type and declaration record.
const MIN_TYPE: usize = 5;
const MIN_DECL: usize = 3 + 20;

fn decode_decl(r: &mut Rd<'_>) -> Res<DeclRec> {
    let code = r.u8()?;
    let flags = r.u8()?;
    let owner = r.u8()?;
    let (name, ctx, lexical, loc, prev) = (r.u32()?, r.u32()?, r.u32()?, r.u32()?, r.u32()?);
    let payload = match code {
        K_NAMESPACE => Payload::Namespace,
        K_EXPORT => Payload::Export,
        K_FUNCTION => {
            let ty = r.q()?;
            let spec = r.u32()?;
            let n = r.count(20)?;
            let mut params = Vec::with_capacity(n);
            for _ in 0..n {
                params.push(ParamRec {
                    name: r.u32()?,
                    loc: r.u32()?,
                    ty: r.q()?,
                    pflags: r.u16()?,
                    flags: r.u8()?,
                    default: r.u32()?,
                });
            }
            let body = match r.u8()? {
                0 => FnBody::None,
                1 => FnBody::Braces(r.u32()?, r.u32()?),
                2 => FnBody::Stmt(r.u32()?),
                _ => return bad("bad flag byte"),
            };
            Payload::Function { ty, spec, params, body }
        }
        K_VAR => Payload::Var { ty: r.q()?, spec: r.u32()?, init: r.u32()?, eval: r.u32()? },
        K_LABEL => Payload::Label,
        K_ENUM => {
            let scoped = r.u8()?;
            let fixed = r.pair()?;
            let def = match r.u8()? {
                0 => None,
                1 => Some(r.pair()?),
                _ => return bad("bad flag byte"),
            };
            Payload::Enum { scoped, fixed, def }
        }
        K_ENUMERATOR => Payload::Enumerator { ty: r.q()?, value: r.i128()? },
        K_TYPEDEF | K_ALIAS => Payload::Typedef { alias: code == K_ALIAS, underlying: r.q()? },
        _ => return bad("unknown declaration kind"),
    };
    Ok(DeclRec { flags, owner, name, ctx, lexical, loc, prev, payload })
}

/// The type references a type record uses, and the record it names.
fn ty_refs(t: &TyRec) -> (Vec<QRef>, Option<u32>) {
    match t {
        TyRec::Pointer(q)
        | TyRec::LRef(q)
        | TyRec::RRef(q)
        | TyRec::CArray(q, _)
        | TyRec::IArray(q)
        | TyRec::Paren(q)
        | TyRec::Elab(_, q)
        | TyRec::QArray(_, q) => (vec![*q], None),
        TyRec::Func { ret, params, .. } => {
            let mut v = Vec::with_capacity(params.len() + 1);
            v.push(*ret);
            v.extend_from_slice(params);
            (v, None)
        }
        TyRec::Enum(d) | TyRec::Typedef(d) => (Vec::new(), Some(*d)),
    }
}

/// The type references a declaration record uses.
fn decl_refs(d: &DeclRec) -> Vec<QRef> {
    match &d.payload {
        Payload::Namespace | Payload::Export | Payload::Label => Vec::new(),
        Payload::Function { ty, params, .. } => {
            std::iter::once(*ty).chain(params.iter().map(|p| p.ty)).collect()
        }
        Payload::Var { ty, .. } | Payload::Enumerator { ty, .. } => vec![*ty],
        Payload::Enum { fixed, def, .. } => {
            let mut v = Vec::new();
            for (a, b) in fixed.iter().chain(def.iter().flatten()) {
                v.push(*a);
                v.push(*b);
            }
            v
        }
        Payload::Typedef { underlying, .. } => vec![*underlying],
    }
}

fn same_kind(a: &Payload, b: &Payload) -> bool {
    a.code() == b.code()
}

/// Decodes and validates an image. Total: any malformed input is an
/// [`ImageError`]; an `Ok` image can be built without further checks
/// other than its offsets against the main file's length.
fn decode(bytes: &[u8]) -> Res<Image> {
    let mut r = Rd(bytes);
    if r.u32()? != IMAGE_VERSION {
        return bad("unsupported version");
    }
    let mut img = Image { main: r.str()?, module: r.str()?, ..Image::default() };
    img.kw = r.u32()?;
    img.name_loc = r.u32()?;
    for which in 0..2 {
        let n = r.count(4)?;
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(r.str()?);
        }
        if which == 0 { img.names = v } else { img.pnames = v }
    }
    let nt = r.count(MIN_TYPE)?;
    img.types = Vec::with_capacity(nt);
    for _ in 0..nt {
        let t = match r.u8()? {
            1 => TyRec::Pointer(r.q()?),
            2 => TyRec::LRef(r.q()?),
            3 => TyRec::RRef(r.q()?),
            4 => TyRec::CArray(r.q()?, r.u64()?),
            5 => TyRec::IArray(r.q()?),
            6 => TyRec::Paren(r.q()?),
            7 => {
                let k = r.u8()?;
                if tag_keyword(k).is_none() {
                    return bad("bad tag keyword");
                }
                TyRec::Elab(k, r.q()?)
            }
            8 => {
                let ret = r.q()?;
                let n = r.count(5)?;
                let mut params = Vec::with_capacity(n);
                for _ in 0..n {
                    params.push(r.q()?);
                }
                let info = r.u16()?;
                let exc = r.u8()?;
                if exc_spec(exc).is_none() {
                    return bad("bad exception specification");
                }
                TyRec::Func { ret, params, info, exc }
            }
            9 => TyRec::Enum(r.u32()?),
            10 => TyRec::Typedef(r.u32()?),
            11 => {
                let c = r.u8()?;
                let q = r.q()?;
                if c == 0 || c > 7 || q.quals != 0 {
                    return bad("bad qualified array");
                }
                TyRec::QArray(c, q)
            }
            _ => return bad("unknown type kind"),
        };
        img.types.push(t);
    }
    let nd = r.count(4 + MIN_DECL)?;
    let mut offsets = Vec::with_capacity(nd);
    for _ in 0..nd {
        offsets.push(r.u32()?);
    }
    let blob_len = r.count(1)?;
    let blob = r.take(blob_len)?;
    let mut br = Rd(blob);
    img.decls = Vec::with_capacity(nd);
    for &o in &offsets {
        if o as usize != blob_len - br.0.len() {
            return bad("bad declaration offset");
        }
        img.decls.push(decode_decl(&mut br)?);
    }
    if !br.0.is_empty() {
        return bad("trailing declaration bytes");
    }
    let nl = r.count(12)?;
    let mut lookup = Vec::with_capacity(nl);
    for _ in 0..nl {
        lookup.push((r.u32()?, r.u32()?, r.u32()?));
    }
    img.bodies = body::decode_bodies(&mut r)?;
    if !r.0.is_empty() {
        return bad("trailing bytes");
    }
    validate(&img, &lookup)?;
    Ok(img)
}

/// The structural checks of [`decode`].
#[allow(clippy::too_many_lines)] // one arm per record or type kind, kept together with the format
fn validate(img: &Image, lookup: &[(u32, u32, u32)]) -> Res<()> {
    let nt = img.types.len() as u64;
    let nd = img.decls.len();
    let qref_ok = |q: QRef, below: u64| -> bool {
        let t = u64::from(q.ty);
        (1..=u64::from(BUILTINS)).contains(&t)
            || (t > u64::from(BUILTINS) && t - u64::from(BUILTINS) - 1 < below)
    };
    // Per type, the highest record (plus one) it or any earlier type
    // names: types are built in order, so a record using type `j` needs
    // every record named by types `0..=j` built before it.
    let mut pdep = Vec::with_capacity(img.types.len());
    let mut acc = 0usize;
    for (j, t) in img.types.iter().enumerate() {
        let (refs, decl) = ty_refs(t);
        if refs.iter().any(|&q| !qref_ok(q, j as u64)) {
            return bad("bad type reference");
        }
        if let Some(d) = decl {
            let ok = img.decls.get(d as usize).is_some_and(|r| {
                matches!(
                    (t, &r.payload),
                    (TyRec::Enum(_), Payload::Enum { .. })
                        | (TyRec::Typedef(_), Payload::Typedef { .. })
                )
            });
            if !ok {
                return bad("bad declaration reference in a type");
            }
            acc = acc.max(d as usize + 1);
        }
        if let TyRec::Func { info, .. } = t
            && FuncInfo::from_bits(*info).bits() != *info
        {
            return bad("bad function type bits");
        }
        pdep.push(acc);
    }
    let qref_at = |q: QRef, i: usize| -> bool {
        if !qref_ok(q, nt) {
            return false;
        }
        let t = u64::from(q.ty);
        t <= u64::from(BUILTINS) || pdep[(t - u64::from(BUILTINS) - 1) as usize] <= i
    };
    let mut open_enum: Option<usize> = None;
    for (i, d) in img.decls.iter().enumerate() {
        let kind_of = |x: u32| img.decls.get(x as usize).filter(|_| (x as usize) < i);
        // A local declaration (a body's variable or label): its context
        // is an earlier function record; it is never looked up by name.
        if kind_of(d.ctx).is_some_and(|c| matches!(c.payload, Payload::Function { .. })) {
            if d.flags & !body::LOCAL_FLAGS != 0
                || d.owner > 3
                || d.lexical != NONE
                || d.prev != NONE
                || (d.name != NONE && d.name as usize >= img.names.len())
            {
                return bad("bad local declaration");
            }
            open_enum = None;
            match &d.payload {
                Payload::Label => {}
                Payload::Var { ty, spec, .. } => {
                    if !qref_at(*ty, i) || var_spec(*spec).is_none() {
                        return bad("bad local variable");
                    }
                }
                _ => return bad("bad local declaration"),
            }
            continue;
        }
        if d.flags & !DECL_FLAGS != 0 || d.owner > 3 || matches!(d.payload, Payload::Label) {
            return bad("bad declaration flags");
        }
        let scope = |x: u32| {
            x == NONE
                || kind_of(x)
                    .is_some_and(|r| matches!(r.payload, Payload::Namespace | Payload::Export))
        };
        let is_enumerator = matches!(d.payload, Payload::Enumerator { .. });
        if is_enumerator {
            if open_enum.is_none() || d.ctx as usize != open_enum.unwrap_or(usize::MAX) {
                return bad("an enumerator outside its enumeration");
            }
        } else {
            open_enum = None;
            if !scope(d.ctx) {
                return bad("bad declaration context");
            }
        }
        if d.lexical != NONE && d.lexical != LEX_TU && !scope(d.lexical) {
            return bad("bad lexical context");
        }
        if d.prev != NONE && !kind_of(d.prev).is_some_and(|p| same_kind(&p.payload, &d.payload)) {
            return bad("bad previous declaration");
        }
        let named = !matches!(d.payload, Payload::Export);
        if named != (d.name != NONE) || (named && d.name as usize >= img.names.len()) {
            return bad("bad declaration name");
        }
        if decl_refs(d).into_iter().any(|q| !qref_at(q, i)) {
            return bad("bad type reference");
        }
        match &d.payload {
            Payload::Function { ty, spec, params, .. } => {
                let proto = (ty.ty > BUILTINS)
                    .then(|| img.types.get((ty.ty - BUILTINS - 1) as usize))
                    .flatten();
                let Some(TyRec::Func { params: pt, .. }) = proto else {
                    return bad("a function without a function type");
                };
                if ty.quals != 0 || pt.len() != params.len() || params.len() > usize::from(u16::MAX)
                {
                    return bad("bad function type");
                }
                if fn_spec(*spec).is_none() {
                    return bad("bad function specifiers");
                }
                for p in params {
                    if (p.name != NONE && p.name as usize >= img.pnames.len())
                        || p.flags & !DECL_FLAGS != 0
                        || p.pflags & !ParamData::SCOPE_DEPTH_MASK != 0
                    {
                        return bad("bad parameter");
                    }
                }
            }
            Payload::Var { spec, .. } => {
                if var_spec(*spec).is_none() {
                    return bad("bad variable specifiers");
                }
            }
            Payload::Enum { scoped, def, .. } => {
                if u32::from(*scoped) & !(EnumData::SCOPED | EnumData::SCOPED_STRUCT) != 0 {
                    return bad("bad enumeration flags");
                }
                if def.is_some() {
                    open_enum = Some(i);
                }
            }
            Payload::Enumerator { ty, .. } => {
                if ty.ty > BUILTINS || ty.quals != 0 {
                    return bad("bad enumerator type");
                }
            }
            Payload::Namespace | Payload::Export | Payload::Typedef { .. } | Payload::Label => {}
        }
    }
    // The lookup table: exactly the named non-local records, sorted.
    let named = img.decls.iter().filter(|d| d.name != NONE && !body::is_local(img, d)).count();
    if lookup.len() != named || lookup.windows(2).any(|w| w[0] >= w[1]) {
        return bad("bad lookup table");
    }
    for &(c, n, d) in lookup {
        if !img.decls.get(d as usize).is_some_and(|r| r.ctx == c && r.name == n) {
            return bad("bad lookup table");
        }
    }
    let _ = nd;
    body::validate_bodies(img, &|q| qref_ok(q, nt))
}

// ---------------------------------------------------------------------------
// The writer
// ---------------------------------------------------------------------------

type W<T> = Result<T, &'static str>;

struct Writer<'s, 'a, 'b> {
    s: &'s Sema<'a, 'b>,
    main: FileId,
    recs: FxHashMap<DeclId, u32>,
    names: FxHashMap<String, u32>,
    pnames: FxHashMap<String, u32>,
    tmemo: FxHashMap<TypeId, u32>,
    img: Image,
    bs: BodyState,
}

/// A reserved identifier ([lex.name]/3): `__x` or `_X`.
fn reserved(s: &str) -> bool {
    let b = s.as_bytes();
    b.first() == Some(&b'_') && b.get(1).is_some_and(|&c| c == b'_' || c.is_ascii_uppercase())
}

impl Writer<'_, '_, '_> {
    fn loc(&self, l: Loc) -> W<u32> {
        if !l.is_valid() {
            return Ok(NONE);
        }
        let sm = self.s.diags.sm();
        match sm.offset_of(l) {
            Some((f, o)) if f == self.main && sm.loc(f, o) == l && o < LEX_TU => Ok(o),
            _ => Err("a declaration outside the module's main file or in a macro expansion"),
        }
    }

    fn name_ix(map: &mut FxHashMap<String, u32>, list: &mut Vec<String>, s: &str) -> u32 {
        if let Some(&i) = map.get(s) {
            return i;
        }
        let i = list.len() as u32;
        list.push(s.to_owned());
        map.insert(s.to_owned(), i);
        i
    }

    fn ident(&self, n: DeclName) -> Option<String> {
        n.as_ident().map(|sym| self.s.ast.symbols().as_str(sym).to_owned())
    }

    fn qref(&mut self, q: QualType) -> W<QRef> {
        let ty = self.ty(q.ty())?;
        let quals = q.quals_bits();
        if quals != 0
            && ty > BUILTINS
            && matches!(
                self.img.types[(ty - BUILTINS - 1) as usize],
                TyRec::LRef(_)
                    | TyRec::RRef(_)
                    | TyRec::CArray(..)
                    | TyRec::IArray(_)
                    | TyRec::Func { .. }
            )
        {
            return Err("a qualified array, reference or function type");
        }
        Ok(QRef { ty, quals })
    }

    /// The record of `d` naming type `t` (its own type, unqualified).
    fn type_decl(&self, d: DeclId, t: TypeId) -> W<u32> {
        let r = self.recs.get(&d).copied().ok_or("a type declared outside the image")?;
        match self.s.ast.decls.get(d).ty() {
            Some(q) if q.ty() == t && q.quals_bits() == 0 => Ok(r),
            _ => Err("a type the image does not carry"),
        }
    }

    fn ty(&mut self, t: TypeId) -> W<u32> {
        if let Some(&i) = self.tmemo.get(&t) {
            return Ok(i);
        }
        let types = &self.s.ast.types;
        let rec = match types.view(t) {
            TypeView::Builtin(b) => {
                let i = b as u32;
                if i == 0 || i > BUILTINS || b.type_id() != t {
                    return Err("a builtin type the image does not carry");
                }
                return Ok(i);
            }
            TypeView::Pointer(q) => TyRec::Pointer(self.qref(q)?),
            TypeView::LValueRef(q) => TyRec::LRef(self.qref(q)?),
            TypeView::RValueRef(q) => TyRec::RRef(self.qref(q)?),
            TypeView::ConstantArray(a) => TyRec::CArray(self.qref(a.elem)?, a.size),
            TypeView::IncompleteArray(q) => TyRec::IArray(self.qref(q)?),
            TypeView::Paren(q) => TyRec::Paren(self.qref(q)?),
            TypeView::Elaborated { keyword, nns: None, named } => {
                TyRec::Elab(keyword as u8, self.qref(named)?)
            }
            TypeView::Function(p) => {
                let exc = match types.exc_spec(&p) {
                    ExcSpec::None => 0,
                    ExcSpec::BasicNoexcept => 1,
                    ExcSpec::DynamicNone => 2,
                    ExcSpec::Nothrow => 3,
                    _ => return Err("an exception specification the image does not carry"),
                };
                let ps: Vec<QualType> = types.params(&p).to_vec();
                let info = p.info().bits();
                let ret = self.qref(p.ret())?;
                let params = ps.into_iter().map(|q| self.qref(q)).collect::<W<Vec<_>>>()?;
                TyRec::Func { ret, params, info, exc }
            }
            TypeView::Enum(d) => TyRec::Enum(self.type_decl(d, t)?),
            TypeView::Typedef { decl, .. } => TyRec::Typedef(self.type_decl(decl, t)?),
            TypeView::QualifiedArray { quals, base } => {
                let sugar = matches!(
                    types.view(base),
                    TypeView::Typedef { .. } | TypeView::Paren(_) | TypeView::Elaborated { .. }
                );
                if !sugar || quals.bits() == 0 {
                    return Err("a qualified array type the image does not carry");
                }
                TyRec::QArray(quals.bits(), self.qref(QualType::unqual(base))?)
            }
            _ => return Err("a type the image does not carry"),
        };
        let i = BUILTINS + 1 + self.img.types.len() as u32;
        self.img.types.push(rec);
        self.tmemo.insert(t, i);
        Ok(i)
    }

    /// Checks the header fields every record shares; returns
    /// (flags, name, ctx, lexical, loc, prev).
    fn head(&mut self, d: DeclId) -> W<(u8, u32, u32, u32, u32, u32)> {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        let extra = n.flags().bits() & !(DECL_FLAGS | DeclFlags::HAS_LEXICAL_CTX.bits());
        if extra != 0 {
            return Err("an implicit, invalid or attributed declaration");
        }
        let m = n.misc();
        if m.access() != Access::None
            || m.friend() != FriendKind::None
            || m.local_extern()
            || m.name_independent()
            || m.module_ownership() != ModuleOwnership::Unowned
        {
            return Err("a declaration with access, friendship or local scope");
        }
        let name = match n.name() {
            x if x == DeclName::EMPTY => NONE,
            x => {
                let s = self.ident(x).ok_or("a declaration not named by an identifier")?;
                if reserved(&s) || s == "std" || s == "main" {
                    return Err("a reserved or special name");
                }
                Self::name_ix(&mut self.names, &mut self.img.names, &s)
            }
        };
        let ctx = match n.ctx() {
            Some(c) if c == ast.tu => NONE,
            Some(c) => {
                self.recs.get(&c).copied().ok_or("a declaration in an unsupported context")?
            }
            None => return Err("a declaration in an unsupported context"),
        };
        let lexical = if n.has(DeclFlags::HAS_LEXICAL_CTX) {
            match ast.decls.lexical_ctx(d) {
                Some(c) if c == ast.tu => LEX_TU,
                Some(c) => self.recs.get(&c).copied().ok_or("an unsupported lexical context")?,
                None => return Err("an unsupported lexical context"),
            }
        } else {
            NONE
        };
        let loc = self.loc(n.loc())?;
        let prev = match ast.decls.redecl(d).and_then(|r| r.prev) {
            None => NONE,
            Some(p) => self
                .recs
                .get(&p)
                .copied()
                .ok_or("a redeclaration of an entity outside the image")?,
        };
        Ok((n.flags().bits() & DECL_FLAGS, name, ctx, lexical, loc, prev))
    }

    #[allow(clippy::too_many_lines)] // one arm per record or type kind, kept together with the format
    /// The payload of record `i`, declaration `d`.
    fn record(&mut self, d: DeclId, i: u32) -> W<Payload> {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        Ok(match n.kind() {
            DeclKind::Namespace => {
                let nd = ast.decls.namespace(d).ok_or("a malformed namespace")?;
                if nd.flags != 0 {
                    return Err("an inline or unnamed namespace");
                }
                Payload::Namespace
            }
            DeclKind::Export => Payload::Export,
            DeclKind::Function => {
                let fd = *ast.decls.function(d).ok_or("a malformed function")?;
                if !fd.templ.is_none() {
                    return Err("a template");
                }
                let s = fd.spec;
                let bits = s.bits();
                if fn_spec(bits) != Some(s) {
                    return Err("a deleted, defaulted, coroutine or contract-checked function");
                }
                if ast.decls.builtin_id(d).is_some() || ast.decls.lib_builtin(d).is_some() {
                    return Err("a builtin or library function");
                }
                let fty = n.ty().ok_or("a malformed function")?;
                if !matches!(ast.types.view(fty.ty()), TypeView::Function(_)) {
                    return Err("a function declared through a typedef");
                }
                let ty = self.qref(fty)?;
                let plist: Vec<DeclId> = ast.lists.iter::<DeclId>(fd.params).collect();
                let mut params = Vec::with_capacity(plist.len());
                for p in plist {
                    let g = self.bs.param_pos.len() as u32;
                    self.bs.param_pos.push((i, params.len() as u32));
                    if self.bs.params.insert(p, g).is_some() {
                        return Err("a parameter shared between declarations");
                    }
                    params.push(self.param(p, g)?);
                }
                let nproto = match self.img.types.get((ty.ty - BUILTINS - 1) as usize) {
                    Some(TyRec::Func { params, .. }) => params.len(),
                    _ => usize::MAX,
                };
                if nproto != params.len() {
                    return Err("a function whose parameters do not match its type");
                }
                let body = match fd.body {
                    None => FnBody::None,
                    Some(b) => {
                        let st = &self.s.ast.stmts;
                        if st.get(b).kind() != StmtKind::CompoundStmt {
                            return Err("a function-try-block");
                        }
                        if Self::carries_body(s) {
                            // Patched by `walk_roots`.
                            self.bs.roots.push(Root::Body(i, b));
                            FnBody::None
                        } else {
                            FnBody::Braces(self.loc(st.get(b).loc())?, self.loc(st.rbrace(b))?)
                        }
                    }
                };
                Payload::Function { ty, spec: bits, params, body }
            }
            DeclKind::Var => self.var_payload(d, i)?,
            DeclKind::Enum => {
                let ed = *ast.decls.enumeration(d).ok_or("a malformed enumeration")?;
                if !ed.templ.is_none() {
                    return Err("a template");
                }
                let defined = ast.definition(d) == Some(d);
                let complete = ed.flags & EnumData::COMPLETE != 0;
                if ed.flags & EnumData::BEING_DEFINED != 0 || complete != defined {
                    return Err("an opaque enumeration declaration");
                }
                let pair = |w: &mut Self| -> W<Option<(QRef, QRef)>> {
                    match (ed.int_type, ed.promotion_type) {
                        (Some(a), Some(b)) => Ok(Some((w.qref(a)?, w.qref(b)?))),
                        _ => Err("an enumeration without an underlying type"),
                    }
                };
                let fixed = if ed.is_fixed() { pair(self)? } else { None };
                let def = match (defined, ed.is_fixed()) {
                    (false, _) => None,
                    (true, true) => Some(None),
                    (true, false) => Some(pair(self)?),
                };
                let scoped = (ed.flags & (EnumData::SCOPED | EnumData::SCOPED_STRUCT)) as u8;
                Payload::Enum { scoped, fixed, def }
            }
            DeclKind::EnumConstant => {
                let ec = *ast.decls.enumerator(d).ok_or("a malformed enumerator")?;
                let parent = n.ctx().ok_or("a malformed enumerator")?;
                if n.ty() != ast.decls.get(parent).ty() {
                    return Err("an enumerator whose type is not its enumeration");
                }
                let v = ec.value.ok_or("an enumerator without a value")?;
                let Some(Scalar::Int { ty, signed, bits }) = ast.consts.scalar(v) else {
                    return Err("an enumerator value the image does not carry");
                };
                let value = if signed {
                    bits.to_i128()
                } else {
                    bits.to_u128().and_then(|u| i128::try_from(u).ok())
                }
                .ok_or("an enumerator value the image does not carry")?;
                let q = self.qref(ty)?;
                if q.ty > BUILTINS || q.quals != 0 {
                    return Err("an enumerator value the image does not carry");
                }
                Payload::Enumerator { ty: q, value }
            }
            DeclKind::Typedef | DeclKind::TypeAlias => {
                let td = *ast.decls.typedef(d).ok_or("a malformed typedef")?;
                if !td.templ.is_none() {
                    return Err("a template");
                }
                Payload::Typedef {
                    alias: n.kind() == DeclKind::TypeAlias,
                    underlying: self.qref(td.underlying)?,
                }
            }
            DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization => return Err("a class"),
            _ => return Err("a declaration the image does not carry yet"),
        })
    }

    /// Parameter `p`, the image's parameter `g`.
    fn param(&mut self, p: DeclId, g: u32) -> W<ParamRec> {
        let ast = &self.s.ast;
        let pd = *ast.decls.param(p).ok_or("a malformed parameter")?;
        let n = ast.decls.get(p);
        if pd.original_ty.is_some() || pd.flags & !ParamData::SCOPE_DEPTH_MASK != 0 {
            return Err("a parameter with an adjusted type or a pack");
        }
        if let Some(e) = pd.default_arg {
            // Patched by `walk_roots`.
            self.bs.roots.push(Root::Default(g, e));
        }
        if n.flags().bits() & !(DECL_FLAGS | DeclFlags::HAS_LEXICAL_CTX.bits()) != 0
            || n.misc().name_independent()
        {
            return Err("an implicit, invalid or attributed parameter");
        }
        let name = match n.name() {
            x if x == DeclName::EMPTY => NONE,
            x => {
                let s = self.ident(x).ok_or("a parameter not named by an identifier")?;
                Self::name_ix(&mut self.pnames, &mut self.img.pnames, &s)
            }
        };
        let loc = self.loc(n.loc())?;
        let ty = self.qref(n.ty().ok_or("a malformed parameter")?)?;
        Ok(ParamRec {
            name,
            loc,
            ty,
            pflags: pd.flags,
            flags: n.flags().bits() & DECL_FLAGS,
            default: NONE,
        })
    }

    /// Whether `d` lies inside a function (a local declaration: it gets a
    /// record only when a carried body names it).
    fn local(&self, d: DeclId) -> bool {
        let decls = &self.s.ast.decls;
        let mut c = decls.get(d).ctx();
        let mut steps = 0u32;
        while let Some(x) = c {
            if decls.function(x).is_some() {
                return true;
            }
            steps += 1;
            if steps > 4096 {
                return false;
            }
            c = decls.get(x).ctx();
        }
        false
    }

    fn collect(&mut self) -> W<()> {
        let s = self.s;
        let ast = &s.ast;
        let mt = &ast.modules;
        let sm = s.diags.sm();
        let unit = mt.units.first().ok_or("no module unit")?;
        if mt.units.len() != 1 || !unit.imports.is_empty() {
            return Err("an import");
        }
        if unit.kind != kiln_ast::modules::UnitKind::Interface || unit.partition.is_some() {
            return Err("a unit that is not a primary module interface");
        }
        self.img.module = unit.module.clone().ok_or("no module declaration")?;
        let (kw, kw_loc, private) = s.module_main_frame().ok_or("no module declaration")?;
        if private {
            return Err("a private module fragment");
        }
        self.img.kw = self.loc(kw)?;
        self.img.name_loc = self.loc(kw_loc)?;
        if self.img.kw == NONE || self.img.name_loc == NONE {
            return Err("no module declaration");
        }
        self.img.main = sm.file_name(self.main).to_owned();
        let mut open_enum: Option<u32> = None;
        for d in ast.decls.ids() {
            let n = ast.decls.get(d);
            let o = mt.owner(d);
            if !o.purview {
                // Predefined declarations have no location in a real
                // file; anything else is a global module fragment.
                let real = sm
                    .offset_of(sm.expansion_loc(n.loc()))
                    .is_some_and(|(f, _)| !crate::bmi_ast::unstable_file(sm.file_name(f)));
                if real && !n.has(DeclFlags::IMPLICIT) {
                    return Err("a global module fragment");
                }
                continue;
            }
            if n.kind() == DeclKind::ParmVar || self.local(d) {
                continue;
            }
            if n.kind() == DeclKind::TranslationUnit {
                continue;
            }
            if n.kind() == DeclKind::StaticAssert {
                // Checked when the module was compiled; nothing to import.
                continue;
            }
            kind_gate(n.kind())?;
            let (flags, name, ctx, lexical, loc, prev) = self.head(d)?;
            let i = self.img.decls.len() as u32;
            let payload = self.record(d, i)?;
            match &payload {
                Payload::Enumerator { .. } => {
                    if open_enum != Some(ctx) {
                        return Err("an enumerator outside its enumeration's definition");
                    }
                }
                Payload::Enum { def: Some(_), .. } => open_enum = Some(i),
                _ => open_enum = None,
            }
            if !matches!(payload, Payload::Enumerator { .. })
                && ctx != NONE
                && !matches!(
                    self.img.decls[ctx as usize].payload,
                    Payload::Namespace | Payload::Export
                )
            {
                return Err("a declaration in an unsupported context");
            }
            let owner = u8::from(o.purview) | (u8::from(o.exported) << 1);
            self.img.decls.push(DeclRec { flags, owner, name, ctx, lexical, loc, prev, payload });
            self.recs.insert(d, i);
        }
        self.walk_roots()
    }
}

/// The declaration kinds the image carries; the fallback reason for the
/// others.
fn kind_gate(k: DeclKind) -> W<()> {
    use DeclKind as K;
    match k {
        K::Namespace
        | K::Export
        | K::Function
        | K::Var
        | K::Enum
        | K::EnumConstant
        | K::Typedef
        | K::TypeAlias => Ok(()),
        K::CXXRecord
        | K::ClassTemplateSpecialization
        | K::ClassTemplatePartialSpecialization
        | K::Field
        | K::IndirectField
        | K::AccessSpec
        | K::Friend
        | K::CXXMethod
        | K::CXXConstructor
        | K::CXXDestructor
        | K::CXXConversion => Err("a class"),
        K::ClassTemplate
        | K::FunctionTemplate
        | K::VarTemplate
        | K::TypeAliasTemplate
        | K::Concept
        | K::VarTemplateSpecialization
        | K::VarTemplatePartialSpecialization
        | K::FriendTemplate
        | K::CXXDeductionGuide
        | K::TemplateTypeParm
        | K::NonTypeTemplateParm
        | K::TemplateTemplateParm => Err("a template"),
        K::NamespaceAlias
        | K::UsingDirective
        | K::Using
        | K::UsingEnum
        | K::UsingShadow
        | K::ConstructorUsingShadow
        | K::UsingPack => Err("a using-declaration, using-directive or namespace alias"),
        K::LinkageSpec => Err("a linkage specification"),
        _ => Err("a declaration the image does not carry yet"),
    }
}

impl Sema<'_, '_> {
    /// Records the main unit's declaration image for a kind-2 module file
    /// (spec/modules.md §11.11 "Part 2a"); set by the driver for
    /// `-fkiln-bmi-image`.
    pub fn record_module_image(&mut self) {
        self.bmi.record_image = true;
    }

    /// The main unit's declaration image ([`Sema::record_module_image`]),
    /// once: the section bytes, or why the unit has none.
    pub fn take_module_image(&mut self) -> Option<ModuleImage> {
        if !std::mem::take(&mut self.bmi.record_image) {
            return None;
        }
        let none = |why| Some(ModuleImage { bytes: Vec::new(), decls: 0, fallback: Some(why) });
        let sm = self.diags.sm();
        let Some((_, kw_loc, _)) = self.module_main_frame() else {
            return none("no module declaration");
        };
        let Some((main, _)) = sm.offset_of(kw_loc) else {
            return none("a module declaration outside a file");
        };
        let mut w = Writer {
            s: self,
            main,
            recs: FxHashMap::default(),
            names: FxHashMap::default(),
            pnames: FxHashMap::default(),
            tmemo: FxHashMap::default(),
            img: Image::default(),
            bs: BodyState::default(),
        };
        if let Err(why) = w.collect() {
            return none(why);
        }
        let decls = w.img.decls.len();
        let bytes = encode(&w.img);
        // The writer's own output must pass the reader's checks (a body
        // shape the schema and the AST disagree on is a fallback, never
        // a corrupt module file).
        if decode(&bytes).is_err() {
            return none("an internal image inconsistency");
        }
        Some(ModuleImage { bytes, decls, fallback: None })
    }

    /// Builds the unit just begun from the declaration image `bytes` of
    /// module file `path`, whose main file the preprocessor loaded as
    /// `main` (spec/modules.md §11.11 "Part 2a"). A malformed image is
    /// the module file's corrupt-file fatal error at `at`, and nothing is
    /// built.
    pub fn load_module_image(&mut self, path: &str, bytes: &[u8], main: FileId, at: Loc) {
        let len = self.diags.sm().text(main).len();
        let checked = decode(bytes).and_then(|img| {
            let off_ok = |o: u32| o == NONE || (o as usize) <= len;
            let mut offs = vec![img.kw, img.name_loc];
            for d in &img.decls {
                offs.push(d.loc);
                if let Payload::Function { params, body, .. } = &d.payload {
                    offs.extend(params.iter().map(|p| p.loc));
                    if let FnBody::Braces(l, r) = body {
                        offs.extend([*l, *r]);
                    }
                }
            }
            body::body_locs(&img, &mut offs);
            if img.kw == NONE || img.name_loc == NONE || !offs.into_iter().all(off_ok) {
                return bad("an offset past the end of the module's source");
            }
            Ok(img)
        });
        match checked {
            Ok(img) => self.build_image(&img, main),
            Err(ImageError(why)) => {
                kiln_diag::Diag::new(
                    kiln_diag::DiagId::pp_module_file_corrupt,
                    kiln_source::Span::new(at, at),
                )
                .arg(path)
                .arg(why)
                .emit(self.diags);
            }
        }
    }

    #[allow(clippy::too_many_lines)] // one arm per record or type kind, kept together with the format
    fn build_image(&mut self, img: &Image, main: FileId) {
        let lo =
            |s: &Self, o: u32| if o == NONE { Loc::INVALID } else { s.diags.sm().loc(main, o) };
        let (kw, name_loc) = (lo(self, img.kw), lo(self, img.name_loc));
        self.act_on_module_directive(
            kw,
            true,
            Directive::Module { name: &img.module, partition: None, kw_loc: name_loc },
        );
        let unit = self.module_current_unit();
        let names: Vec<DeclName> =
            img.names.iter().map(|n| DeclName::ident(self.intern(n))).collect();
        let pnames: Vec<DeclName> =
            img.pnames.iter().map(|n| DeclName::ident(self.intern(n))).collect();
        let mut ids: Vec<DeclId> = Vec::with_capacity(img.decls.len());
        let mut tys: Vec<QualType> = Vec::with_capacity(img.types.len());
        let mut open: Option<(DeclId, Option<(QRef, QRef)>)> = None;
        // Parameters in image order; what is attached once the bodies
        // are built: function bodies, variable initializers and values.
        let mut params_all: Vec<DeclId> = Vec::new();
        let mut fbodies: Vec<(DeclId, u32)> = Vec::new();
        let mut inits: Vec<(DeclId, u32, u32)> = Vec::new();
        for (i, r) in img.decls.iter().enumerate() {
            if !matches!(r.payload, Payload::Enumerator { .. })
                && let Some((e, computed)) = open.take()
            {
                self.image_complete_enum(img, &mut tys, &ids, e, computed);
            }
            let owner = Owner { unit, purview: r.owner & 1 != 0, exported: r.owner & 2 != 0 };
            let next = self.ast.decls.next_id();
            self.ast.modules.set_owner_from(next, owner);
            let at = |x: u32| if x == NONE { None } else { ids.get(x as usize).copied() };
            let ctx = at(r.ctx).unwrap_or(self.ast.tu);
            let name = if r.name == NONE { DeclName::EMPTY } else { names[r.name as usize] };
            let mut h = DeclHead::new(ctx, name, lo(self, r.loc));
            if r.flags & DeclFlags::IMPLICIT.bits() != 0 {
                h.flags = DeclFlags::IMPLICIT;
            }
            h.lexical = match r.lexical {
                NONE => None,
                LEX_TU => Some(self.ast.tu),
                x => at(x),
            };
            let prev = at(r.prev);
            let d = match &r.payload {
                Payload::Namespace => self.builder().namespace(h, false, prev),
                Payload::Export => self.builder().export_decl(ctx, h.loc),
                Payload::Function { ty, spec, params, body } => {
                    let fty = self.image_ty(img, &mut tys, &ids, *ty);
                    let spec = fn_spec(*spec).unwrap_or_default();
                    let f = self.builder().function(h, DeclKind::Function, fty, spec, prev);
                    let mut ps = Vec::with_capacity(params.len());
                    for (k, p) in params.iter().enumerate() {
                        let pty = self.image_ty(img, &mut tys, &ids, p.ty);
                        let pname =
                            if p.name == NONE { DeclName::EMPTY } else { pnames[p.name as usize] };
                        let ploc = lo(self, p.loc);
                        let index = u16::try_from(k).unwrap_or(u16::MAX);
                        let pd = self.builder().param(f, pname, ploc, pty, index, p.pflags);
                        image_flags(self, pd, p.flags);
                        ps.push(pd);
                    }
                    if !ps.is_empty() {
                        let _ = self.builder().set_params(f, &ps);
                    }
                    params_all.extend_from_slice(&ps);
                    match *body {
                        FnBody::None => {}
                        FnBody::Braces(lb, rb) => {
                            let (lb, rb) = (lo(self, lb), lo(self, rb));
                            let b = self.builder().compound(&[], lb, rb);
                            let _ = self.builder().set_body(f, b);
                        }
                        FnBody::Stmt(s) => fbodies.push((f, s)),
                    }
                    f
                }
                Payload::Var { ty, spec, init, eval } => {
                    let vty = self.image_ty(img, &mut tys, &ids, *ty);
                    let spec = var_spec(*spec).unwrap_or_default();
                    let v = self.builder().var(h, vty, spec, prev);
                    inits.push((v, *init, *eval));
                    v
                }
                Payload::Label => self.image_label(ctx, name, h.loc),
                Payload::Enum { scoped, fixed, def } => {
                    let fixed = fixed.map(|(a, b)| {
                        (
                            self.image_ty(img, &mut tys, &ids, a),
                            self.image_ty(img, &mut tys, &ids, b),
                        )
                    });
                    let e = self.builder().enumeration(h, u32::from(*scoped), fixed, prev);
                    if let Some(computed) = def {
                        let _ = self.builder().begin_enum_definition(e);
                        open = Some((e, *computed));
                    }
                    e
                }
                Payload::Enumerator { ty, value } => {
                    let vty = self.image_ty(img, &mut tys, &ids, *ty);
                    let d = self.builder().enumerator(h, None);
                    let cv = self.int_const(vty, *value);
                    let _ = self.ast.decls.set_enumerator_value(d, cv);
                    d
                }
                Payload::Typedef { alias, underlying } => {
                    let u = self.image_ty(img, &mut tys, &ids, *underlying);
                    let kind = if *alias { DeclKind::TypeAlias } else { DeclKind::Typedef };
                    self.builder().typedef(h, kind, u, prev)
                }
            };
            image_flags(self, d, r.flags);
            debug_assert_eq!(ids.len(), i);
            ids.push(d);
        }
        if let Some((e, computed)) = open.take() {
            self.image_complete_enum(img, &mut tys, &ids, e, computed);
        }
        let built = self.image_bodies(img, &mut tys, &ids, &params_all, main);
        for (f, s) in fbodies {
            let _ = self.builder().set_body(f, built.stmt(s));
        }
        for (v, init, eval) in inits {
            if init != NONE {
                let _ = self.builder().set_var_init(v, built.expr(init));
            }
            if eval != NONE && self.ast.decls.begin_eval(v).is_ok() {
                let value = (eval != body::NOT_CONST).then(|| built.consts[eval as usize]);
                self.ast.decls.finish_eval(v, value);
            }
        }
        for (g, p) in img.decls.iter().flat_map(|d| match &d.payload {
            Payload::Function { params, .. } => params.as_slice(),
            _ => &[],
        }).enumerate() {
            if p.default != NONE {
                let _ = self.ast.decls.set_default_arg(params_all[g], built.expr(p.default));
            }
        }
        self.module_owner_changed();
    }

    fn image_complete_enum(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        e: DeclId,
        computed: Option<(QRef, QRef)>,
    ) {
        let computed = computed
            .map(|(a, b)| (self.image_ty(img, tys, ids, a), self.image_ty(img, tys, ids, b)));
        let _ = self.builder().complete_enum_definition(e, computed);
    }

    /// The type `q` names, building image types in order up to it (each
    /// refers only to earlier ones and to records already built, which
    /// [`decode`] checked).
    fn image_ty(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        q: QRef,
    ) -> QualType {
        let base = if q.ty <= BUILTINS {
            BuiltinType::from_index(q.ty).map_or(QualType::INT, BuiltinType::qual_type)
        } else {
            let j = (q.ty - BUILTINS - 1) as usize;
            while tys.len() <= j {
                let t = self.image_build_ty(&img.types[tys.len()], tys, ids);
                tys.push(t);
            }
            tys[j]
        };
        if q.quals == 0 { base } else { self.ast.types.qualify(base, Quals::from_bits(q.quals)) }
    }

    fn image_build_ty(&mut self, t: &TyRec, tys: &[QualType], ids: &[DeclId]) -> QualType {
        let qt = |s: &mut Self, q: QRef| -> QualType {
            let base = if q.ty <= BUILTINS {
                BuiltinType::from_index(q.ty).map_or(QualType::INT, BuiltinType::qual_type)
            } else {
                tys[(q.ty - BUILTINS - 1) as usize]
            };
            if q.quals == 0 { base } else { s.ast.types.qualify(base, Quals::from_bits(q.quals)) }
        };
        let decl_ty = |s: &Self, d: u32| {
            ids.get(d as usize).and_then(|&x| s.ast.decls.get(x).ty()).unwrap_or(QualType::INT)
        };
        match t {
            TyRec::Pointer(q) => {
                let q = qt(self, *q);
                self.ast.types.pointer_to(q)
            }
            TyRec::LRef(q) => {
                let q = qt(self, *q);
                self.ast.types.lvalue_ref_to(q)
            }
            TyRec::RRef(q) => {
                let q = qt(self, *q);
                self.ast.types.rvalue_ref_to(q)
            }
            TyRec::CArray(q, n) => {
                let q = qt(self, *q);
                self.ast.types.constant_array(q, *n, None)
            }
            TyRec::IArray(q) => {
                let q = qt(self, *q);
                self.ast.types.incomplete_array(q)
            }
            TyRec::Paren(q) => {
                let q = qt(self, *q);
                self.ast.types.paren_type(q)
            }
            TyRec::Elab(k, q) => {
                let q = qt(self, *q);
                let k = tag_keyword(*k).unwrap_or_default();
                self.ast.types.elaborated(k, None, q)
            }
            TyRec::Func { ret, params, info, exc } => {
                let r = qt(self, *ret);
                let ps: Vec<QualType> = params.iter().map(|&p| qt(self, p)).collect();
                let exc = exc_spec(*exc).unwrap_or_default();
                self.ast.types.function(r, &ps, FuncInfo::from_bits(*info), exc)
            }
            TyRec::Enum(d) | TyRec::Typedef(d) => decl_ty(self, *d),
            TyRec::QArray(c, q) => {
                let q = qt(self, *q);
                self.ast.types.qualify(q, Quals::from_bits(*c))
            }
        }
    }
}

fn image_flags(s: &mut Sema<'_, '_>, d: DeclId, flags: u8) {
    for f in [DeclFlags::USED, DeclFlags::REFERENCED] {
        if flags & f.bits() != 0 {
            s.ast.decls.set_flags(d, f);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_lines)] // a literal image covering every record and type kind
    fn sample() -> Image {
        let q = |ty| QRef { ty, quals: 0 };
        let int = BuiltinType::Int as u32;
        Image {
            main: "m.cppm".into(),
            module: "m".into(),
            kw: 0,
            name_loc: 7,
            names: ["ns", "f", "E", "a", "T", "v", "g", "y", "L"].map(String::from).to_vec(),
            pnames: vec!["x".into(), "p".into()],
            types: vec![
                TyRec::Func { ret: q(int), params: vec![q(int)], info: 0, exc: 0 },
                TyRec::Enum(3),
                TyRec::Pointer(QRef { ty: BUILTINS + 2, quals: 1 }),
            ],
            decls: vec![
                DeclRec {
                    flags: 0,
                    owner: 1,
                    name: 0,
                    ctx: NONE,
                    lexical: NONE,
                    loc: 20,
                    prev: NONE,
                    payload: Payload::Namespace,
                },
                DeclRec {
                    flags: 0,
                    owner: 1,
                    name: NONE,
                    ctx: 0,
                    lexical: NONE,
                    loc: 30,
                    prev: NONE,
                    payload: Payload::Export,
                },
                DeclRec {
                    flags: 4,
                    owner: 3,
                    name: 1,
                    ctx: 1,
                    lexical: NONE,
                    loc: 40,
                    prev: NONE,
                    payload: Payload::Function {
                        ty: q(BUILTINS + 1),
                        spec: 0,
                        params: vec![ParamRec {
                            name: 0,
                            loc: 46,
                            ty: q(int),
                            pflags: 0x100,
                            flags: 0,
                            default: NONE,
                        }],
                        body: FnBody::Braces(50, 51),
                    },
                },
                DeclRec {
                    flags: 0,
                    owner: 3,
                    name: 2,
                    ctx: 1,
                    lexical: NONE,
                    loc: 60,
                    prev: NONE,
                    payload: Payload::Enum {
                        scoped: 1,
                        fixed: Some((q(int), q(int))),
                        def: Some(None),
                    },
                },
                DeclRec {
                    flags: 0,
                    owner: 3,
                    name: 3,
                    ctx: 3,
                    lexical: NONE,
                    loc: 70,
                    prev: NONE,
                    payload: Payload::Enumerator { ty: q(int), value: -3 },
                },
                DeclRec {
                    flags: 0,
                    owner: 3,
                    name: 4,
                    ctx: 1,
                    lexical: NONE,
                    loc: 80,
                    prev: NONE,
                    payload: Payload::Typedef { alias: true, underlying: q(BUILTINS + 3) },
                },
                DeclRec {
                    flags: 0,
                    owner: 1,
                    name: 5,
                    ctx: 0,
                    lexical: LEX_TU,
                    loc: 90,
                    prev: NONE,
                    payload: Payload::Var { ty: q(BUILTINS + 3), spec: 2, init: NONE, eval: NONE },
                },
                // constexpr int g(int p = 7) { int y = p; return 7 + y; } L:
                DeclRec {
                    flags: 0,
                    owner: 3,
                    name: 6,
                    ctx: 1,
                    lexical: NONE,
                    loc: 100,
                    prev: NONE,
                    payload: Payload::Function {
                        ty: q(BUILTINS + 1),
                        spec: 0x18,
                        params: vec![ParamRec {
                            name: 1,
                            loc: 102,
                            ty: q(int),
                            pflags: 0x100,
                            flags: 4,
                            default: 0,
                        }],
                        body: FnBody::Stmt(2),
                    },
                },
                DeclRec {
                    flags: 4,
                    owner: 3,
                    name: 7,
                    ctx: 7,
                    lexical: NONE,
                    loc: 106,
                    prev: NONE,
                    payload: Payload::Var { ty: q(int), spec: 0, init: 1, eval: NONE },
                },
                DeclRec {
                    flags: 0,
                    owner: 3,
                    name: 8,
                    ctx: 7,
                    lexical: NONE,
                    loc: 130,
                    prev: NONE,
                    payload: Payload::Label,
                },
            ],
            bodies: Bodies {
                consts: vec![body::ConstRec::Int {
                    ty: q(int),
                    signed: true,
                    width: 32,
                    words: vec![7],
                }],
                strings: vec![(0, b"hi".to_vec())],
                exprs: vec![
                    // 7 (a wide literal)
                    body::ExprRec {
                        kind: 0,
                        bits: 0x10,
                        aux: 0,
                        cold: 0,
                        ty: q(int),
                        loc: 101,
                        words: vec![0],
                    },
                    // p
                    body::ExprRec {
                        kind: 7,
                        bits: 1,
                        aux: 0,
                        cold: 1,
                        ty: q(int),
                        loc: 107,
                        words: vec![body::PARAM, 0, NONE, 108],
                    },
                    // 7 + y (the operands' shapes only matter to Sema)
                    body::ExprRec {
                        kind: 10,
                        bits: 0,
                        aux: 0,
                        cold: 0,
                        ty: q(int),
                        loc: 110,
                        words: vec![0, 1],
                    },
                ],
                stmts: vec![
                    body::StmtRec { kind: 1, aux: 0, loc: 105, words: vec![1, 8] },
                    body::StmtRec { kind: 14, aux: 0, loc: 109, words: vec![2, NONE] },
                    body::StmtRec { kind: 0, aux: 0, loc: 104, words: vec![2, 0, 1, 120] },
                ],
            },
        }
    }

    #[test]
    fn round_trip() {
        let img = sample();
        let bytes = encode(&img);
        assert_eq!(decode(&bytes), Ok(img));
    }

    #[test]
    fn rejects_out_of_order_references() {
        let mut img = sample();
        // A type naming a record that follows its first user.
        img.decls.swap(2, 3);
        assert!(decode(&encode(&img)).is_err());
        let mut img = sample();
        img.decls[4].ctx = 2; // an enumerator outside its enumeration
        assert!(decode(&encode(&img)).is_err());
        let mut img = sample();
        img.decls[2].prev = 1; // a function redeclaring an export
        assert!(decode(&encode(&img)).is_err());
        let mut img = sample();
        img.bodies.exprs[2].words = vec![2, 1]; // an operand naming itself
        assert!(decode(&encode(&img)).is_err());
        let mut img = sample();
        img.bodies.stmts[0].words = vec![1, 6]; // a declaration statement of a global
        assert!(decode(&encode(&img)).is_err());
        let mut img = sample();
        img.bodies.stmts[2].words = vec![2, 0, 2, 120]; // a block containing itself
        assert!(decode(&encode(&img)).is_err());
        let mut img = sample();
        if let Payload::Function { body, .. } = &mut img.decls[7].payload {
            *body = FnBody::Stmt(1); // not a compound statement
        }
        assert!(decode(&encode(&img)).is_err());
        let mut img = sample();
        img.decls[9].ctx = 0; // a label outside a function
        assert!(decode(&encode(&img)).is_err());
    }


    /// [module.import] / spec/modules.md §11.11 "Part 2a": a damaged image
    /// (truncated, flipped bytes, corrupt lengths) is an error, never a
    /// panic.
    #[test]
    fn fuzz_never_panics() {
        let bytes = encode(&sample());
        for n in 0..bytes.len() {
            assert!(decode(&bytes[..n]).is_err(), "prefix {n} decoded");
        }
        for i in 0..bytes.len() {
            for x in [0x01u8, 0x80, 0xff, 0x7f, 0x10] {
                let mut b = bytes.clone();
                b[i] ^= x;
                let _ = decode(&b);
            }
        }
        for i in 0..bytes.len().saturating_sub(3) {
            for v in [u32::MAX, u32::MAX / 2, 0x0100_0000, 0, NONE - 1] {
                let mut b = bytes.clone();
                b[i..i + 4].copy_from_slice(&v.to_le_bytes());
                let _ = decode(&b);
            }
        }
        let mut s: u64 = 0x9e37_79b9_7f4a_7c15;
        for _ in 0..4000 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let mut b = bytes.clone();
            let i = (s as usize) % b.len();
            b[i] = (s >> 32) as u8;
            let j = ((s >> 16) as usize) % b.len();
            b.truncate(j.max(i));
            let _ = decode(&b);
        }
    }
}
