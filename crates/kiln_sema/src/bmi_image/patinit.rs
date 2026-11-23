//! Mem-initializers of constructors in class templates in the
//! declaration image (spec/modules.md §11.11, "Pattern mem-initializers";
//! WP-1310, WP-3026).
//!
//! A constructor definition in a template keeps its mem-initializers as
//! written on the `Ast` (`Ast::pattern_mem_inits`, spec/ast.md §8.6), not
//! as `CtorInit`s: each instantiation substitutes into them and checks
//! the result as the parser's action would ([temp.inst]/4,
//! [class.base.init]). The section lists, per such constructor record,
//! the `:` location and each mem-initializer: its mem-initializer-id
//! (an identifier, a member by its name, the type the definition
//! resolved, with the identifier it was written as when each
//! instantiation looks it up again), its written range, whether it is a
//! pack expansion (`B(args)...`, [temp.variadic]/5) and its initializer
//! (`( expression-list )` or a braced-init-list, decomposed into its
//! clauses and nested lists). The expressions are records of the bodies
//! section (`Root::PatInit`). The importer installs the record on the
//! constructor once the bodies are built, before anything can
//! instantiate it: a member is found again by its name in the imported
//! class, a braced list rebuilt as the definition built it.
//!
//! Not carried (the constructor falls back): a list with a mem-initializer
//! whose definition-time check failed (`PatternMemInits::failed`, already
//! diagnosed), and a braced-init-list with designators.

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::templates::{PatternMemInit, PatternMemInitArgs, PatternMemInitId, PatternMemInits};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_source::symbol::Symbol;

use super::body::{Built, Root};
use super::{Image, Locs, NONE, Payload, QRef, Rd, Res, W, Wr, Writer, bad};
use crate::Sema;
use crate::init::{InitItem, InitList, InitValue};
use crate::lookup::LookupKind;

/// The deepest braced-init-list nesting the encoder and the decoder
/// accept.
const MAX_DEPTH: u32 = 64;

/// One mem-initializer-id.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum IdRec {
    /// An identifier (an index of the image's `pnames`).
    Ident(u32),
    /// A type bound at the definition.
    Type(QRef),
    /// A non-static data member, by its name (`pnames`).
    Member(u32),
    /// A type written as an identifier (`pnames`) looked up again.
    NamedType(QRef, u32),
}

/// A braced-init-list: its clauses (an expression slot or a nested
/// list), `{` and `}`.
#[derive(Clone, PartialEq, Eq, Debug)]
struct ListRec {
    items: Vec<ItemRec>,
    lbrace: u32,
    rbrace: u32,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum ItemRec {
    Expr(u32),
    List(ListRec),
}

/// An initializer; expressions are slots of [`PatInits::exprs`].
#[derive(Clone, PartialEq, Eq, Debug)]
enum InitRec {
    Paren(Vec<u32>, u32, u32),
    List(ListRec),
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct MemInitRec {
    id: IdRec,
    loc: u32,
    end: u32,
    pack: bool,
    init: InitRec,
}

/// One constructor's list.
#[derive(Clone, PartialEq, Eq, Debug)]
struct PatInit {
    /// The constructor's record.
    rec: u32,
    /// The `:`, [`NONE`] when no ctor-initializer is written.
    colon: u32,
    inits: Vec<MemInitRec>,
}

/// The section.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct PatInits {
    ctors: Vec<PatInit>,
    /// The expression records the lists name, patched by `walk_roots`.
    pub(super) exprs: Vec<u32>,
}

impl Writer<'_, '_, '_> {
    /// Lists the pattern mem-initializers of constructor `d`, record `i`
    /// (a definition in a class template), if it has a record (every
    /// pattern constructor has one, the empty list included).
    pub(super) fn pattern_mem_inits(&mut self, d: DeclId, i: u32) -> W<()> {
        let s = self.s;
        let Some(p) = s.ast.pattern_mem_inits(d) else { return Ok(()) };
        if p.failed {
            return Err("a mem-initializer that failed its definition-time check");
        }
        // No ctor-initializer: `NONE` (an image location's "invalid").
        let colon = p.colon.map_or(Ok(NONE), |c| self.loc(c))?;
        let mut inits = Vec::with_capacity(p.inits.len());
        for m in &p.inits {
            let id = match m.id {
                PatternMemInitId::Ident(sym) => IdRec::Ident(self.pat_name(sym)),
                PatternMemInitId::Member(f) => {
                    let sym = s.ast.decls.get(f).name().as_ident();
                    IdRec::Member(self.pat_name(sym.ok_or("a malformed pattern mem-initializer")?))
                }
                PatternMemInitId::Type { ty, ident: None } => IdRec::Type(self.qref(ty)?),
                PatternMemInitId::Type { ty, ident: Some(sym) } => {
                    let q = self.qref(ty)?;
                    IdRec::NamedType(q, self.pat_name(sym))
                }
            };
            let (loc, end) = (self.loc(m.loc)?, self.loc(m.end)?);
            let init = match &m.init {
                PatternMemInitArgs::Paren { args, lparen, rparen } => {
                    let slots = args.iter().map(|&e| self.pat_expr(e)).collect();
                    InitRec::Paren(slots, self.loc(*lparen)?, self.loc(*rparen)?)
                }
                PatternMemInitArgs::List(e) => InitRec::List(self.pat_list(*e, 0)?),
            };
            inits.push(MemInitRec { id, loc, end, pack: m.pack_expansion, init });
        }
        self.img.patinits.ctors.push(PatInit { rec: i, colon, inits });
        Ok(())
    }

    fn pat_name(&mut self, sym: Symbol) -> u32 {
        let t = self.s.ast.symbols().as_str(sym).to_owned();
        Self::name_ix(&mut self.pnames, &mut self.img.pnames, &t)
    }

    fn pat_expr(&mut self, e: ExprId) -> u32 {
        let k = self.img.patinits.exprs.len() as u32;
        self.img.patinits.exprs.push(NONE);
        self.bs.roots.push(Root::PatInit(k, e));
        k
    }

    /// The type-`void` `InitListExpr` `e` of a pattern mem-initializer,
    /// decomposed: a nested `InitListExpr` is a nested list, any other
    /// clause an expression slot.
    fn pat_list(&mut self, e: ExprId, depth: u32) -> W<ListRec> {
        let ex = &self.s.ast.exprs;
        if depth > MAX_DEPTH || ex.get(e).kind() != ExprKind::InitListExpr {
            return Err("a malformed pattern mem-initializer");
        }
        let lbrace = ex.get(e).loc();
        let rbrace = ex.end_loc(e).unwrap_or(lbrace);
        let clauses: Vec<ExprId> = self.s.ast.lists.iter::<ExprId>(ex.init_list_inits(e)).collect();
        let mut items = Vec::with_capacity(clauses.len());
        for x in clauses {
            let ex = &self.s.ast.exprs;
            let designated = |y: ExprId| ex.get(y).kind() == ExprKind::DesignatedInitExpr;
            if designated(x) || crate::templ::pack::expr_pattern(self.s, x).is_some_and(designated)
            {
                return Err("a designated initializer in a mem-initializer");
            }
            items.push(if ex.get(x).kind() == ExprKind::InitListExpr {
                ItemRec::List(self.pat_list(x, depth + 1)?)
            } else {
                ItemRec::Expr(self.pat_expr(x))
            });
        }
        Ok(ListRec { items, lbrace: self.loc(lbrace)?, rbrace: self.loc(rbrace)? })
    }
}

fn encode_list(w: &mut Wr, l: &ListRec) {
    w.len(l.items.len());
    for it in &l.items {
        match it {
            ItemRec::Expr(e) => (w.u8(0), w.u32(*e)).1,
            ItemRec::List(n) => (w.u8(1), encode_list(w, n)).1,
        }
    }
    w.u32(l.lbrace);
    w.u32(l.rbrace);
}

pub(super) fn encode_pattern_inits(w: &mut Wr, s: &PatInits) {
    w.len(s.ctors.len());
    for c in &s.ctors {
        w.u32(c.rec);
        w.u32(c.colon);
        w.len(c.inits.len());
        for m in &c.inits {
            match m.id {
                IdRec::Ident(n) => (w.u8(0), w.u32(n)).1,
                IdRec::Type(q) => (w.u8(1), w.q(q)).1,
                IdRec::Member(n) => (w.u8(2), w.u32(n)).1,
                IdRec::NamedType(q, n) => {
                    w.u8(3);
                    w.q(q);
                    w.u32(n);
                }
            }
            w.u32(m.loc);
            w.u32(m.end);
            w.u8(u8::from(m.pack));
            match &m.init {
                InitRec::Paren(es, l, r) => {
                    w.u8(1);
                    w.len(es.len());
                    for &e in es {
                        w.u32(e);
                    }
                    w.u32(*l);
                    w.u32(*r);
                }
                InitRec::List(l) => (w.u8(2), encode_list(w, l)).1,
            }
        }
    }
    w.len(s.exprs.len());
    for &e in &s.exprs {
        w.u32(e);
    }
}

fn decode_list(r: &mut Rd, depth: u32) -> Res<ListRec> {
    if depth > MAX_DEPTH {
        return bad("bad pattern mem-initializer");
    }
    let n = r.count(5)?;
    let mut items = Vec::with_capacity(n);
    for _ in 0..n {
        items.push(match r.u8()? {
            0 => ItemRec::Expr(r.u32()?),
            1 => ItemRec::List(decode_list(r, depth + 1)?),
            _ => return bad("bad pattern mem-initializer"),
        });
    }
    Ok(ListRec { items, lbrace: r.u32()?, rbrace: r.u32()? })
}

pub(super) fn decode_pattern_inits(r: &mut Rd) -> Res<PatInits> {
    let n = r.count(12)?;
    let mut ctors = Vec::with_capacity(n);
    for _ in 0..n {
        let (rec, colon) = (r.u32()?, r.u32()?);
        let m = r.count(15)?;
        let mut inits = Vec::with_capacity(m);
        for _ in 0..m {
            let id = match r.u8()? {
                0 => IdRec::Ident(r.u32()?),
                1 => IdRec::Type(r.q()?),
                2 => IdRec::Member(r.u32()?),
                3 => IdRec::NamedType(r.q()?, r.u32()?),
                _ => return bad("bad pattern mem-initializer"),
            };
            let (loc, end) = (r.u32()?, r.u32()?);
            let pack = match r.u8()? {
                0 => false,
                1 => true,
                _ => return bad("bad pattern mem-initializer"),
            };
            let init = match r.u8()? {
                1 => {
                    let k = r.count(4)?;
                    let mut es = Vec::with_capacity(k);
                    for _ in 0..k {
                        es.push(r.u32()?);
                    }
                    InitRec::Paren(es, r.u32()?, r.u32()?)
                }
                2 => InitRec::List(decode_list(r, 0)?),
                _ => return bad("bad pattern mem-initializer"),
            };
            inits.push(MemInitRec { id, loc, end, pack, init });
        }
        ctors.push(PatInit { rec, colon, inits });
    }
    let k = r.count(4)?;
    let mut exprs = Vec::with_capacity(k);
    for _ in 0..k {
        exprs.push(r.u32()?);
    }
    Ok(PatInits { ctors, exprs })
}

fn list_locs(l: &ListRec, offs: &mut Vec<u32>) {
    offs.extend([l.lbrace, l.rbrace]);
    for it in &l.items {
        if let ItemRec::List(n) = it {
            list_locs(n, offs);
        }
    }
}

/// The section's locations, for the decoder's offset check.
pub(super) fn pattern_init_locs(img: &Image, offs: &mut Vec<u32>) {
    for c in &img.patinits.ctors {
        offs.push(c.colon);
        for m in &c.inits {
            offs.extend([m.loc, m.end]);
            match &m.init {
                InitRec::Paren(_, l, r) => offs.extend([*l, *r]),
                InitRec::List(l) => list_locs(l, offs),
            }
        }
    }
}

fn list_ok(l: &ListRec, slot_ok: &dyn Fn(u32) -> bool) -> bool {
    l.items.iter().all(|it| match it {
        ItemRec::Expr(e) => slot_ok(*e),
        ItemRec::List(n) => list_ok(n, slot_ok),
    })
}

/// Checks the section: each entry names a distinct constructor record,
/// in ascending order; every slot is an expression of the bodies, every
/// identifier a name and every type a type reference of the image
/// (`qref_ok`).
pub(super) fn validate_pattern_inits(img: &Image, qref_ok: &dyn Fn(QRef) -> bool) -> Res<()> {
    let p = &img.patinits;
    let nexpr = img.bodies.exprs.len();
    if !p.exprs.iter().all(|&e| (e as usize) < nexpr) {
        return bad("bad pattern mem-initializer");
    }
    let slot_ok = |k: u32| (k as usize) < p.exprs.len();
    let name_ok = |n: u32| (n as usize) < img.pnames.len();
    let mut last: Option<u32> = None;
    for c in &p.ctors {
        let ctor = matches!(
            img.decls.get(c.rec as usize).map(|d| &d.payload),
            Some(Payload::Function { kind: 2, .. })
        );
        let ok = ctor
            && last.is_none_or(|l| l < c.rec)
            && c.inits.iter().all(|m| {
                let id_ok = match m.id {
                    IdRec::Ident(n) | IdRec::Member(n) => name_ok(n),
                    IdRec::Type(q) => qref_ok(q),
                    IdRec::NamedType(q, n) => qref_ok(q) && name_ok(n),
                };
                id_ok
                    && match &m.init {
                        InitRec::Paren(es, _, _) => es.iter().all(|&e| slot_ok(e)),
                        InitRec::List(l) => list_ok(l, &slot_ok),
                    }
            });
        if !ok {
            return bad("bad pattern mem-initializer");
        }
        last = Some(c.rec);
    }
    Ok(())
}

/// The importer's side.
impl Sema<'_, '_> {
    /// Installs each constructor's pattern mem-initializers (after the
    /// bodies) on the `Ast`, as the definition left them.
    pub(super) fn image_pattern_inits(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        main: &Locs,
        built: &Built,
    ) {
        let p = &img.patinits;
        let slot = |k: u32| built.expr(p.exprs[k as usize]);
        for c in &p.ctors {
            let ctor = ids[c.rec as usize];
            if self.kind_of(ctor) != DeclKind::CXXConstructor {
                continue;
            }
            let mut list = Vec::with_capacity(c.inits.len());
            for m in &c.inits {
                let id = match m.id {
                    IdRec::Ident(n) => {
                        PatternMemInitId::Ident(self.intern(&img.pnames[n as usize]))
                    }
                    IdRec::Type(q) => {
                        PatternMemInitId::Type { ty: self.image_ty(img, tys, ids, q), ident: None }
                    }
                    IdRec::NamedType(q, n) => {
                        let ty = self.image_ty(img, tys, ids, q);
                        PatternMemInitId::Type {
                            ty,
                            ident: Some(self.intern(&img.pnames[n as usize])),
                        }
                    }
                    IdRec::Member(n) => {
                        let name = self.intern(&img.pnames[n as usize]);
                        match self.image_pattern_member(ctor, name) {
                            Some(f) => PatternMemInitId::Member(f),
                            None => PatternMemInitId::Ident(name),
                        }
                    }
                };
                let init = match &m.init {
                    InitRec::Paren(es, l, r) => PatternMemInitArgs::Paren {
                        args: es.iter().map(|&e| slot(e)).collect(),
                        lparen: main.get(*l),
                        rparen: main.get(*r),
                    },
                    InitRec::List(l) => {
                        let written = image_list(l, main, &slot);
                        PatternMemInitArgs::List(self.dependent_init_list(&written))
                    }
                };
                list.push(PatternMemInit {
                    id,
                    loc: main.get(m.loc),
                    end: main.get(m.end),
                    pack_expansion: m.pack,
                    init,
                });
            }
            let colon = (c.colon != NONE).then(|| main.get(c.colon));
            let rec = PatternMemInits { colon, failed: false, inits: list.into() };
            let _ = self.builder().set_pattern_mem_inits(ctor, rec);
        }
    }

    /// The non-static data member named `name` of constructor `ctor`'s
    /// class, as the definition's lookup found it.
    fn image_pattern_member(&mut self, ctor: DeclId, name: Symbol) -> Option<DeclId> {
        let class = self.ast.decls.get(ctor).ctx()?;
        let def = self.class_def(class);
        let found = self.lookup_in(def, name, LookupKind::Ordinary);
        found
            .decls
            .iter()
            .copied()
            .find(|&d| matches!(self.kind_of(d), DeclKind::Field | DeclKind::IndirectField))
    }
}

fn image_list(l: &ListRec, main: &Locs, slot: &dyn Fn(u32) -> ExprId) -> InitList {
    let items = l
        .items
        .iter()
        .map(|it| InitItem {
            designator: Vec::new(),
            value: match it {
                ItemRec::Expr(e) => InitValue::Expr(slot(*e)),
                ItemRec::List(n) => InitValue::List(image_list(n, main, slot)),
            },
        })
        .collect();
    InitList { items, lbrace: main.get(l.lbrace), rbrace: main.get(l.rbrace) }
}
