//! Scopes and names of the declaration image beyond named namespaces and
//! export blocks (spec/modules.md §11.11, "Scopes and reserved names",
//! WP-1278): inline and unnamed namespaces, linkage specifications, the
//! namespace `std` sema may already have declared, and reserved names.
//!
//! A linkage specification is an `Export` record (transparent, unnamed,
//! a context like an export block) listed in [`Scopes::links`] with its
//! language; an inline namespace is a `Namespace` record listed in
//! [`Scopes::inline`]. The importer builds a `LinkageSpec` or an inline
//! namespace for them. An unnamed namespace is a `Namespace` record with
//! no name; the importer extends the unnamed namespace its scope already
//! has ([namespace.unnamed]/1, as the parser does) or declares it with
//! its implicit using-directive, which the writer leaves out.
//!
//! Sema declares two kinds of global-scope names by itself, which the
//! name gate cannot see (§11.11 "ODR merging"): builtins (a lazily
//! declared `__builtin_*` function, a predeclared `__builtin_va_list`)
//! and the hidden namespace `std` around an implicit `std::align_val_t`.
//! A declaration at global scope with a builtin's name, or of a name sema
//! predeclares there, falls back. The namespace `std` is carried: the
//! importer extends its implicit one, if it has one, as the parser's
//! first `namespace std {` does, and the unit's `std::align_val_t`
//! redeclares the implicit enumeration there.
//!
//! A builtin function sema declared lazily while compiling the unit (an
//! implicit `Function` in an implicit `extern "C"` linkage specification
//! at global scope) is an `Implicit` record of kind [`BUILTIN_KIND`],
//! unnamed and at global scope, listed in [`Scopes::builtins`] with its
//! builtin id and spelling; its linkage specification has no record. The
//! importer finds the builtin it has declared already or declares it
//! (as its lookup of the name would), so bodies that call it call the
//! importer's.
//!
//! An unnamed class or enumeration that a typedef names for linkage
//! purposes ([dcl.typedef]/9, `typedef struct { } T;`) is listed in
//! [`Scopes::anon_typedefs`] with the typedef; the importer links them
//! (`typedef_for_anon`) as soon as it builds the typedef, so linkage and
//! mangling see the typedef name.

use kiln_ast::decl::{DeclFlags, DeclKind, LinkageLang, NamespaceData, ScopeData, builtin};
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::{DeclHead, DeclId, QualType};
use kiln_source::Loc;

use super::{DECL_FLAGS, DeclRec, Image, NONE, Payload, Rd, Res, W, Wr, Writer, bad};
use crate::Sema;

/// A linkage specification record: the `Export` record `rec`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct LinkRec {
    rec: u32,
    /// `LinkageLang` (1 C, 2 C++).
    lang: u8,
    /// Written without braces (`extern "C" int x;`).
    braceless: bool,
}

/// The `Implicit` record kind of a lazily declared builtin function
/// (special members are 0 to 5).
pub(super) const BUILTIN_KIND: u8 = 6;

/// The `Implicit` record kind of one of the implicitly declared global
/// allocation and deallocation functions ([basic.stc.dynamic.general]/2,
/// WP-1320).
pub(super) const GLOBAL_OP_KIND: u8 = 7;

/// The `Implicit` record kind of the implicitly declared
/// `std::align_val_t` ([new.syn], [basic.stc.dynamic.general]/2; WP-1353),
/// which a type names when no declaration of the program precedes the
/// first new-expression or allocation function declaration.
pub(super) const ALIGN_VAL_T_KIND: u8 = 10;

/// An implicit global allocation or deallocation function: `Implicit`
/// record `rec`, the `index`th implicit `operator op` sema declares
/// (`declare_global_new_delete`, in declaration order).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct GlobalOpStub {
    rec: u32,
    /// `OverloadedOperator` (`new`, `new[]`, `delete`, `delete[]`).
    op: u8,
    index: u8,
}

/// A lazily declared builtin function: `Implicit` record `rec`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct BuiltinStub {
    rec: u32,
    id: u32,
    /// The builtin's spelling, checked against the importer's table.
    name: String,
}

/// The scope section.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct Scopes {
    /// Inline namespace records, ascending.
    inline: Vec<u32>,
    /// Linkage specification records, ascending.
    links: Vec<LinkRec>,
    /// Builtin function records, ascending.
    builtins: Vec<BuiltinStub>,
    /// (unnamed class or enumeration, the typedef naming it for linkage
    /// purposes) records, ascending by typedef.
    anon_typedefs: Vec<(u32, u32)>,
    /// Implicit global allocation and deallocation function records,
    /// ascending (WP-1320).
    global_ops: Vec<GlobalOpStub>,
}

/// `d` is an implicit `extern "C"` linkage specification at global
/// scope: where sema declares a builtin function lazily.
fn builtin_scope(s: &Sema<'_, '_>, d: DeclId) -> bool {
    let ast = &s.ast;
    let n = ast.decls.get(d);
    n.kind() == DeclKind::LinkageSpec
        && n.has(DeclFlags::IMPLICIT)
        && n.ctx() == Some(ast.tu)
        && ast.decls.scope(d).and_then(ScopeData::linkage_lang) == Some(LinkageLang::C)
}

/// The implicit global `operator` functions named `name`, in declaration
/// order.
fn implicit_global_ops(s: &Sema<'_, '_>, name: DeclName) -> Vec<DeclId> {
    let ast = &s.ast;
    let mut v: Vec<DeclId> = ast
        .lookup_local(ast.tu, name)
        .iter()
        .filter(|&f| {
            let n = ast.decls.get(f);
            n.kind() == DeclKind::Function && n.has(DeclFlags::IMPLICIT)
        })
        .collect();
    v.sort_unstable();
    v
}

/// The operator of a replaceable global allocation or deallocation
/// function's name ([new.delete], WP-1320).
fn global_new_delete_op(name: DeclName) -> Option<OverloadedOperator> {
    use OverloadedOperator as O;
    name.as_operator().filter(|op| matches!(op, O::New | O::ArrayNew | O::Delete | O::ArrayDelete))
}

/// `d`'s name is declared at global namespace scope: its context is the
/// translation unit, seen through export blocks, linkage specifications,
/// unnamed namespaces and unscoped enumerations (the scopes the gate's
/// `gated_names` sees through).
fn global_scope(s: &Sema<'_, '_>, d: DeclId) -> bool {
    let ast = &s.ast;
    let mut c = ast.decls.get(d).ctx();
    for _ in 0..4096 {
        let Some(x) = c else { return false };
        let n = ast.decls.get(x);
        let through = match n.kind() {
            DeclKind::TranslationUnit => return true,
            DeclKind::Export | DeclKind::LinkageSpec => true,
            DeclKind::Namespace => n.name() == DeclName::EMPTY,
            DeclKind::Enum => ast.decls.enumeration(x).is_some_and(|e| !e.is_scoped()),
            _ => false,
        };
        if !through {
            return false;
        }
        c = n.ctx();
    }
    false
}

impl Writer<'_, '_, '_> {
    /// Checks identifier `name` of `d` against the names sema declares by
    /// itself.
    pub(super) fn check_name(&self, d: DeclId, name: &str) -> W<()> {
        let reserved = super::reserved(name);
        if !reserved && name != "main" {
            return Ok(());
        }
        if !global_scope(self.s, d) {
            return Ok(());
        }
        if name == "main" {
            return Err("a declaration named `main` at global scope");
        }
        if builtin::by_name(name).is_some() {
            return Err("a reserved name of a builtin at global scope");
        }
        let ast = &self.s.ast;
        let sym = ast.symbols().get(name);
        let predeclared = sym.is_some_and(|sym| {
            ast.lookup_local(ast.tu, DeclName::ident(sym))
                .iter()
                .any(|p| ast.decls.get(p).has(DeclFlags::IMPLICIT))
        });
        if predeclared {
            return Err("a reserved name sema predeclares at global scope");
        }
        Ok(())
    }

    /// `p`, the previous declaration of `d`, is one the importer's sema
    /// declares by itself and the reader finds again ([`Sema::image_prev`]):
    /// the implicit namespace `std` or its `std::align_val_t`, or an
    /// implicit global allocation or deallocation function (WP-1320,
    /// [`Sema::image_global_new_delete_prev`]).
    pub(super) fn implicit_prev(&self, d: DeclId, p: DeclId) -> bool {
        let ast = &self.s.ast;
        if ast.decls.get(d).kind() == DeclKind::Function {
            let n = ast.decls.get(p);
            return n.kind() == DeclKind::Function
                && n.has(DeclFlags::IMPLICIT)
                && n.ctx() == Some(ast.tu)
                && global_new_delete_op(n.name()).is_some();
        }
        match ast.decls.get(d).kind() {
            DeclKind::Namespace => self.s.global_ops.implicit_std == Some(ast.first(p)),
            // WP-1353: the implicit `std::align_val_t` is made in the
            // program's `namespace std` when one was declared before the
            // first new-expression or allocation function declaration
            // (`std_align_val_t`), else in the implicit one.
            DeclKind::Enum => {
                let n = ast.decls.get(p);
                n.has(DeclFlags::IMPLICIT)
                    && n.ctx().is_some_and(|c| is_global_std(ast, c))
                    && ast
                        .decls
                        .get(d)
                        .name()
                        .as_ident()
                        .is_some_and(|s| ast.symbols().as_str(s) == "align_val_t")
            }
            _ => false,
        }
    }

    /// The namespace flags of record `i`, namespace `d`.
    pub(super) fn namespace_flags(&mut self, i: u32, flags: u32) -> W<()> {
        if flags & !(NamespaceData::INLINE | NamespaceData::ANONYMOUS) != 0 {
            return Err("a namespace with flags the image does not carry");
        }
        if flags & NamespaceData::INLINE != 0 {
            self.img.scopes.inline.push(i);
        }
        Ok(())
    }

    /// The record of linkage specification `d`, record `i`.
    pub(super) fn linkage_record(&mut self, d: DeclId, i: u32) -> W<Payload> {
        let ast = &self.s.ast;
        let sd = ast.decls.scope(d).ok_or("a malformed linkage specification")?;
        let lang = sd.linkage_lang().ok_or("a malformed linkage specification")?;
        if lang == LinkageLang::C && !self.pch && !global_scope(self.s, d) {
            // [dcl.link]/6: a C function or variable in a namespace is the
            // same entity as one of its name at global scope. A precompiled
            // header's reader records them in the consumer's extern C
            // context (`image_extern_c`, WP-1310); a module's does not.
            return Err("a C linkage specification in a namespace");
        }
        let braceless = sd.aux & ScopeData::LINKAGE_BRACELESS != 0;
        self.img.scopes.links.push(LinkRec { rec: i, lang: lang as u8, braceless });
        Ok(Payload::Export)
    }

    /// Writes implicit declaration `d` if it is a lazily declared builtin
    /// function or the linkage specification around one (which has no
    /// record); false for any other declaration.
    pub(super) fn builtin_record(&mut self, d: DeclId) -> W<bool> {
        let s = self.s;
        let ast = &s.ast;
        let n = ast.decls.get(d);
        if builtin_scope(s, d) {
            return Ok(true);
        }
        if !n.ctx().is_some_and(|c| builtin_scope(s, c)) {
            return Ok(false);
        }
        let not = "an implicit declaration in a linkage specification that is not a builtin";
        let id = ast.decls.builtin_id(d).filter(|_| n.kind() == DeclKind::Function).ok_or(not)?;
        let info = builtin::info(id).ok_or(not)?;
        let spelled = n.name().as_ident().is_some_and(|x| ast.symbols().as_str(x) == info.name);
        if !spelled || builtin::by_name_at(info.name, s.clang_compat) != Some(id) {
            return Err(not);
        }
        // Declared only while a `FILE` type is (`lazily_create_builtin`),
        // which the importer may not have where the record is.
        if info.sig.contains('P') {
            return Err("a builtin function whose type names `FILE`");
        }
        let o = ast.modules.owner(d);
        let rec = self.img.decls.len() as u32;
        self.img.decls.push(DeclRec {
            flags: n.flags().bits() & DECL_FLAGS,
            owner: u8::from(o.purview) | (u8::from(o.exported) << 1),
            access: 0,
            name: NONE,
            ctx: NONE,
            lexical: NONE,
            loc: NONE,
            prev: NONE,
            payload: Payload::Implicit { kind: BUILTIN_KIND },
        });
        self.img.scopes.builtins.push(BuiltinStub { rec, id, name: info.name.to_owned() });
        self.recs.insert(d, rec);
        Ok(true)
    }

    /// The record of `d` when it is one of the implicitly declared
    /// global allocation and deallocation functions (WP-1320): a stub
    /// the importer resolves to its own after declaring them.
    pub(super) fn global_op_stub(&mut self, d: DeclId) -> W<Option<u32>> {
        let s = self.s;
        let ast = &s.ast;
        let n = ast.decls.get(d);
        let Some(op) = global_new_delete_op(n.name()) else { return Ok(None) };
        if n.kind() != DeclKind::Function || !n.has(DeclFlags::IMPLICIT) || n.ctx() != Some(ast.tu)
        {
            return Ok(None);
        }
        if let Some(&r) = self.recs.get(&d) {
            return Ok(Some(r));
        }
        let index = implicit_global_ops(s, n.name())
            .iter()
            .position(|&x| x == d)
            .and_then(|i| u8::try_from(i).ok())
            .ok_or("an implicit allocation function sema did not declare")?;
        let o = ast.modules.owner(d);
        let rec = self.img.decls.len() as u32;
        self.img.decls.push(DeclRec {
            flags: n.flags().bits() & DECL_FLAGS,
            owner: u8::from(o.purview) | (u8::from(o.exported) << 1),
            access: 0,
            name: NONE,
            ctx: NONE,
            lexical: NONE,
            loc: NONE,
            prev: NONE,
            payload: Payload::Implicit { kind: GLOBAL_OP_KIND },
        });
        self.img.scopes.global_ops.push(GlobalOpStub { rec, op: op as u8, index });
        self.recs.insert(d, rec);
        Ok(Some(rec))
    }

    /// The stub record of `d` when it is sema's implicit
    /// `std::align_val_t` (WP-1353), written on first use: the importer
    /// declares the global allocation functions there and takes its own.
    pub(super) fn align_val_t_stub(&mut self, d: DeclId) -> Option<u32> {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        if n.kind() != DeclKind::Enum
            || !n.has(DeclFlags::IMPLICIT)
            || !n.ctx().is_some_and(|c| is_global_std(ast, c))
            || self.s.global_ops.align_val_t.is_none()
        {
            return None;
        }
        if let Some(&r) = self.recs.get(&d) {
            return Some(r);
        }
        let rec = self.img.decls.len() as u32;
        self.img.decls.push(DeclRec {
            flags: 0,
            owner: 0,
            access: 0,
            name: NONE,
            ctx: NONE,
            lexical: NONE,
            loc: NONE,
            prev: NONE,
            payload: Payload::Implicit { kind: ALIGN_VAL_T_KIND },
        });
        self.recs.insert(d, rec);
        Some(rec)
    }

    /// Lists the carried unnamed classes and enumerations that a typedef
    /// names for linkage purposes; the typedef must be carried too.
    pub(super) fn anon_typedefs(&mut self) -> W<()> {
        let mut pairs = Vec::new();
        let mut missing = Vec::new();
        for (tag, &td) in self.s.ast.decls.cold.typedef_for_anon.iter() {
            let Some(&t) = self.recs.get(&tag) else { continue };
            match self.recs.get(&td) {
                Some(&r) => pairs.push((t, r)),
                None => missing.push(t),
            }
        }
        missing.sort_unstable();
        for t in missing {
            self.miss(t, "a class named for linkage by a typedef outside the image")?;
        }
        pairs.sort_unstable_by_key(|&(_, td)| td);
        self.img.scopes.anon_typedefs = pairs;
        Ok(())
    }

    /// An implicit declaration the importer declares again itself: the
    /// using-directive that follows an unnamed namespace.
    pub(super) fn implicit_scope_decl(&self, d: DeclId) -> bool {
        let ast = &self.s.ast;
        ast.decls.get(d).kind() == DeclKind::UsingDirective
            && ast.using_target(d).is_some_and(|ns| {
                let n = ast.decls.get(ns);
                n.kind() == DeclKind::Namespace && n.name() == DeclName::EMPTY
            })
    }
}

pub(super) fn encode_scopes(w: &mut Wr, s: &Scopes) {
    w.len(s.inline.len());
    for &r in &s.inline {
        w.u32(r);
    }
    w.len(s.links.len());
    for l in &s.links {
        w.u32(l.rec);
        w.u8(l.lang);
        w.u8(u8::from(l.braceless));
    }
    w.len(s.builtins.len());
    for b in &s.builtins {
        w.u32(b.rec);
        w.u32(b.id);
        w.str(&b.name);
    }
    w.len(s.anon_typedefs.len());
    for &(t, td) in &s.anon_typedefs {
        w.u32(t);
        w.u32(td);
    }
    w.len(s.global_ops.len());
    for g in &s.global_ops {
        w.u32(g.rec);
        w.u8(g.op);
        w.u8(g.index);
    }
}

pub(super) fn decode_scopes(r: &mut Rd) -> Res<Scopes> {
    let mut s = Scopes::default();
    let n = r.count(4)?;
    s.inline = Vec::with_capacity(n);
    for _ in 0..n {
        s.inline.push(r.u32()?);
    }
    let n = r.count(6)?;
    s.links = Vec::with_capacity(n);
    for _ in 0..n {
        let rec = r.u32()?;
        let lang = r.u8()?;
        let braceless = match r.u8()? {
            0 => false,
            1 => true,
            _ => return bad("a bad linkage specification"),
        };
        s.links.push(LinkRec { rec, lang, braceless });
    }
    let n = r.count(12)?;
    s.builtins = Vec::with_capacity(n);
    for _ in 0..n {
        let (rec, id) = (r.u32()?, r.u32()?);
        s.builtins.push(BuiltinStub { rec, id, name: r.str()? });
    }
    let n = r.count(8)?;
    s.anon_typedefs = Vec::with_capacity(n);
    for _ in 0..n {
        s.anon_typedefs.push((r.u32()?, r.u32()?));
    }
    let n = r.count(6)?;
    s.global_ops = Vec::with_capacity(n);
    for _ in 0..n {
        let (rec, op, index) = (r.u32()?, r.u8()?, r.u8()?);
        s.global_ops.push(GlobalOpStub { rec, op, index });
    }
    Ok(s)
}

pub(super) fn validate_scopes(img: &Image) -> Res<()> {
    let s = &img.scopes;
    let rec = |r: u32| img.decls.get(r as usize);
    if !s.inline.windows(2).all(|w| w[0] < w[1])
        || !s
            .inline
            .iter()
            .all(|&r| rec(r).is_some_and(|d| matches!(d.payload, Payload::Namespace)))
    {
        return bad("a bad inline namespace");
    }
    if !s.links.windows(2).all(|w| w[0].rec < w[1].rec)
        || !s.links.iter().all(|l| {
            matches!(l.lang, 1 | 2)
                && rec(l.rec).is_some_and(|d| matches!(d.payload, Payload::Export))
        })
    {
        return bad("a bad linkage specification");
    }
    let stub =
        |r: u32| matches!(rec(r), Some(d) if d.payload == Payload::Implicit { kind: BUILTIN_KIND });
    let stubs = img.decls.iter().filter(|d| d.payload == Payload::Implicit { kind: BUILTIN_KIND });
    if !s.builtins.windows(2).all(|w| w[0].rec < w[1].rec)
        || stubs.count() != s.builtins.len()
        || !s
            .builtins
            .iter()
            .all(|b| stub(b.rec) && builtin::info(b.id).is_some_and(|i| i.name == b.name))
    {
        return bad("a bad builtin function record");
    }
    let tag = |t: u32| {
        matches!(rec(t), Some(d) if d.name == NONE
            && matches!(d.payload, Payload::Record { .. } | Payload::Enum { .. }))
    };
    let typedef =
        |r: u32| matches!(rec(r), Some(d) if matches!(d.payload, Payload::Typedef { .. }));
    if !s.anon_typedefs.windows(2).all(|w| w[0].1 < w[1].1)
        || !s.anon_typedefs.iter().all(|&(t, td)| t < td && tag(t) && typedef(td))
    {
        return bad("a bad typedef naming an unnamed class");
    }
    let op_stub = |r: u32| matches!(rec(r), Some(d) if d.payload == Payload::Implicit { kind: GLOBAL_OP_KIND });
    let op_stubs =
        img.decls.iter().filter(|d| d.payload == Payload::Implicit { kind: GLOBAL_OP_KIND });
    if !s.global_ops.windows(2).all(|w| w[0].rec < w[1].rec)
        || op_stubs.count() != s.global_ops.len()
        || !s.global_ops.iter().all(|g| {
            op_stub(g.rec)
                && OverloadedOperator::from_u8(g.op)
                    .and_then(|o| global_new_delete_op(DeclName::operator(o)))
                    .is_some()
        })
    {
        return bad("a bad implicit allocation function record");
    }
    Ok(())
}

impl Sema<'_, '_> {
    /// Whether the importer declares every builtin function the image
    /// names (the same table, compatibility level and target).
    pub(super) fn image_builtins_ok(&self, img: &Image) -> bool {
        img.scopes.builtins.iter().all(|b| {
            builtin::by_name_at(&b.name, self.clang_compat) == Some(b.id)
                && self.target_has_builtin(b.id)
        })
    }

    /// The importer's declaration of builtin function record `i`: the
    /// one its lookup found or declared before, else a new one.
    pub(super) fn image_builtin(&mut self, img: &Image, i: u32) -> DeclId {
        let tu = self.ast.tu;
        let Ok(k) = img.scopes.builtins.binary_search_by_key(&i, |b| b.rec) else { return tu };
        let b = &img.scopes.builtins[k];
        let sym = self.intern(&b.name);
        let found = self.ast.lookup_local(tu, DeclName::ident(sym)).iter().find(|&p| {
            self.ast.decls.get(p).has(DeclFlags::IMPLICIT)
                && self.ast.decls.builtin_id(p) == Some(b.id)
        });
        found.or_else(|| self.lazily_create_builtin(sym, Loc::INVALID)).unwrap_or(tu)
    }

    /// The importer's implicit global allocation or deallocation
    /// function for stub record `i` (WP-1320): declared as a
    /// new-expression would, then the same one by position.
    /// The importer's implicit `std::align_val_t` (stub kind
    /// [`ALIGN_VAL_T_KIND`]): declared with the global allocation
    /// functions, as a new-expression would.
    pub(super) fn image_align_val_t(&mut self) -> DeclId {
        self.declare_global_new_delete();
        let tu = self.ast.tu;
        let Some(q) = self.global_ops.align_val_t else { return tu };
        let c = self.ast.types.canonical(q);
        match self.ast.types.view(c.ty()) {
            kiln_ast::types::TypeView::Enum(d) => d,
            _ => tu,
        }
    }

    pub(super) fn image_global_op(&mut self, img: &Image, i: u32) -> DeclId {
        let tu = self.ast.tu;
        let Ok(k) = img.scopes.global_ops.binary_search_by_key(&i, |g| g.rec) else { return tu };
        let g = img.scopes.global_ops[k];
        let Some(op) = OverloadedOperator::from_u8(g.op) else { return tu };
        self.declare_global_new_delete();
        implicit_global_ops(self, DeclName::operator(op))
            .get(usize::from(g.index))
            .copied()
            .unwrap_or(tu)
    }

    /// Links typedef record `i`, just built, to the unnamed class or
    /// enumeration it names for linkage purposes, if any.
    pub(super) fn image_anon_typedef(&mut self, img: &Image, i: u32, ids: &[DeclId]) {
        let list = &img.scopes.anon_typedefs;
        if let Ok(k) = list.binary_search_by_key(&i, |&(_, td)| td) {
            let (t, td) = (ids[list[k].0 as usize], ids[i as usize]);
            let _ = self.ast.decls.set_typedef_for_anon(t, td);
        }
    }

    /// Builds namespace record `i` (header `h`, previous `prev`).
    pub(super) fn image_namespace(
        &mut self,
        img: &Image,
        i: u32,
        h: DeclHead,
        mut prev: Option<DeclId>,
    ) -> DeclId {
        let inline = img.scopes.inline.binary_search(&i).is_ok();
        let tu = self.ast.tu;
        let at_tu = || h.ctx == tu;
        let anon = h.name == DeclName::EMPTY;
        let key = self.ast.primary_context(self.ast.redecl_context(h.ctx));
        if prev.is_none() && anon {
            prev = self.anon_namespaces.get(&key).copied();
        }
        if prev.is_none()
            && at_tu()
            && h.name.as_ident().is_some_and(|s| self.ast.symbols().as_str(s) == "std")
        {
            prev = self.global_ops.implicit_std;
        }
        let inline = inline
            || prev.is_some_and(|p| {
                self.ast.decls.namespace(self.ast.first(p)).is_some_and(NamespaceData::is_inline)
            });
        let ns = self.builder().namespace(h, inline, prev);
        if anon {
            self.anon_namespaces.entry(key).or_insert(ns);
            if prev.is_none() {
                let _ = self.using_directive(ns, h.loc, DeclFlags::IMPLICIT);
            }
        }
        ns
    }

    /// The previous declaration of function record `h` (type `ty`, no
    /// previous record): a replaceable global allocation or deallocation
    /// function redeclares the implicit one sema declares first, as
    /// `act_on_global_new_delete_decl` finds it ([new.delete]/2,
    /// WP-1320). Every global `operator new` / `operator delete` goes
    /// through it, as in the parser, so the implicit functions are
    /// declared at the same point.
    pub(super) fn image_global_new_delete_prev(
        &mut self,
        h: &DeclHead,
        kind: DeclKind,
        ty: QualType,
    ) -> Option<DeclId> {
        let op = global_new_delete_op(h.name)?;
        let ctx = self.ast.redecl_context(h.ctx);
        if kind != DeclKind::Function || self.ast.decls.get(ctx).kind() != DeclKind::TranslationUnit
        {
            return None;
        }
        self.act_on_global_new_delete_decl(op, ty).1
    }

    /// The previous declaration of enumeration record `i` (header `h`)
    /// sema declared by itself: the implicit `std::align_val_t` in the
    /// implicit namespace `std` that `h.ctx` extends.
    pub(super) fn image_enum_prev(&self, h: &DeclHead) -> Option<DeclId> {
        // The implicit namespace or the program's (WP-1353): sema makes
        // the implicit enumeration in whichever `std` lookup found first.
        if !is_global_std(&self.ast, h.ctx) {
            return None;
        }
        let std = self.ast.first(h.ctx);
        let sym = h.name.as_ident().filter(|&s| self.ast.symbols().as_str(s) == "align_val_t")?;
        self.ast.lookup_local(std, DeclName::ident(sym)).iter().find(|&p| {
            let n = self.ast.decls.get(p);
            n.kind() == DeclKind::Enum && n.has(DeclFlags::IMPLICIT)
        })
    }

    /// Builds `Export` record `i` (context `ctx`, header `h`): a linkage
    /// specification when the scope section lists it.
    pub(super) fn image_scope(&mut self, img: &Image, i: u32, ctx: DeclId, h: DeclHead) -> DeclId {
        let Ok(k) = img.scopes.links.binary_search_by_key(&i, |l| l.rec) else {
            return self.builder().export_decl(ctx, h.loc);
        };
        let l = img.scopes.links[k];
        let lang = if l.lang == 1 { LinkageLang::C } else { LinkageLang::Cxx };
        let ls = self.builder().linkage_spec_head(DeclHead::new(ctx, DeclName::EMPTY, h.loc), lang);
        if l.braceless {
            self.ast.decls.set_linkage_braceless(ls);
        }
        ls
    }
}

/// Whether record `i`, an [`ALIGN_VAL_T_KIND`] stub, is the image's only
/// one.
pub(super) fn align_val_t_stub_ok(img: &Image, i: usize) -> bool {
    let stub = |d: &DeclRec| d.payload == Payload::Implicit { kind: ALIGN_VAL_T_KIND };
    img.decls.iter().position(stub) == Some(i) && img.decls.iter().filter(|d| stub(d)).count() == 1
}

/// Whether `ns` is a declaration of namespace `std` at global scope (an
/// implicit one included), where sema declares `std::align_val_t`.
pub(super) fn is_global_std(ast: &kiln_ast::Ast, ns: DeclId) -> bool {
    let n = ast.decls.get(ns);
    n.kind() == DeclKind::Namespace
        && n.ctx() == Some(ast.tu)
        && n.name().as_ident().is_some_and(|s| ast.symbols().as_str(s) == "std")
}

#[cfg(test)]
mod tests {
    use super::super::tests::sample;
    use super::super::{TyRec, decode, encode};
    use super::*;

    /// `sample` with a builtin function record appended.
    fn with_builtin(name: &str) -> Image {
        let mut img = sample();
        let rec = img.decls.len() as u32;
        img.decls.push(DeclRec {
            flags: DeclFlags::IMPLICIT.bits() & DECL_FLAGS,
            owner: 1,
            access: 0,
            name: NONE,
            ctx: NONE,
            lexical: NONE,
            loc: NONE,
            prev: NONE,
            payload: Payload::Implicit { kind: BUILTIN_KIND },
        });
        let id = builtin::by_name(name).unwrap();
        img.scopes.builtins.push(BuiltinStub { rec, id, name: name.to_owned() });
        img
    }

    /// spec/modules.md §11.11 "Scopes and reserved names": a builtin
    /// function record round-trips; one the builtin list does not name
    /// consistently, or not at global scope, is an error.
    #[test]
    fn builtin_records() {
        type Corrupt = (&'static str, fn(&mut Image));
        let img = with_builtin("__builtin_strlen");
        assert_eq!(decode(&encode(&img)).as_ref(), Ok(&img));
        let corrupt: [Corrupt; 4] = [
            ("wrong spelling", |i| i.scopes.builtins[0].name = "__builtin_abs".into()),
            ("unlisted", |i| i.scopes.builtins.clear()),
            ("not a builtin record", |i| i.scopes.builtins[0].rec = 0),
            ("in a namespace", |i| i.decls.last_mut().unwrap().ctx = 0),
        ];
        for (what, f) in corrupt {
            let mut img = with_builtin("__builtin_strlen");
            f(&mut img);
            assert!(decode(&encode(&img)).is_err(), "{what}");
        }
    }

    /// WP-1353: the implicit `std::align_val_t`'s stub, named by an enum
    /// type, round-trips; a second one, or one with a name or a previous
    /// record, is an error.
    #[test]
    fn align_val_t_records() {
        type Corrupt = (&'static str, fn(&mut Image));
        let with = || {
            let mut img = sample();
            let rec = img.decls.len() as u32;
            img.decls.push(DeclRec {
                flags: 0,
                owner: 0,
                access: 0,
                name: NONE,
                ctx: NONE,
                lexical: NONE,
                loc: NONE,
                prev: NONE,
                payload: Payload::Implicit { kind: ALIGN_VAL_T_KIND },
            });
            img.types.push(TyRec::Enum(rec));
            img
        };
        let img = with();
        assert_eq!(decode(&encode(&img)).as_ref(), Ok(&img));
        let corrupt: [Corrupt; 3] = [
            ("twice", |i| {
                let d = i.decls.last().unwrap().clone();
                i.decls.push(d);
            }),
            ("named", |i| i.decls.last_mut().unwrap().name = 0),
            ("a redeclaration", |i| i.decls.last_mut().unwrap().prev = 0),
        ];
        for (what, f) in corrupt {
            let mut img = with();
            f(&mut img);
            assert!(decode(&encode(&img)).is_err(), "{what}");
        }
    }

    /// WP-1320: an implicit global allocation function record
    /// round-trips; one not listed, listed for another operator, or not
    /// at global scope, is an error.
    #[test]
    fn global_op_records() {
        type Corrupt = (&'static str, fn(&mut Image));
        let with = || {
            let mut img = sample();
            let rec = img.decls.len() as u32;
            img.decls.push(DeclRec {
                flags: DeclFlags::IMPLICIT.bits() & DECL_FLAGS,
                owner: 0,
                access: 0,
                name: NONE,
                ctx: NONE,
                lexical: NONE,
                loc: NONE,
                prev: NONE,
                payload: Payload::Implicit { kind: GLOBAL_OP_KIND },
            });
            let op = OverloadedOperator::New as u8;
            img.scopes.global_ops.push(GlobalOpStub { rec, op, index: 0 });
            img
        };
        let img = with();
        assert_eq!(decode(&encode(&img)).as_ref(), Ok(&img));
        let corrupt: [Corrupt; 4] = [
            ("unlisted", |i| i.scopes.global_ops.clear()),
            ("not a stub", |i| i.scopes.global_ops[0].rec = 0),
            ("not an allocation function", |i| {
                i.scopes.global_ops[0].op = OverloadedOperator::Plus as u8;
            }),
            ("in a namespace", |i| i.decls.last_mut().unwrap().ctx = 0),
        ];
        for (what, f) in corrupt {
            let mut img = with();
            f(&mut img);
            assert!(decode(&encode(&img)).is_err(), "{what}");
        }
    }
}
