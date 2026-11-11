//! Mem-initializers of constructors in class templates in the
//! declaration image (spec/modules.md §11.11, "Pattern mem-initializers";
//! WP-1310).
//!
//! A constructor definition in a class template keeps its
//! mem-initializers as parsed (`InstState::pattern_mem_inits`), not as
//! `CtorInit`s of the declaration: each instantiation substitutes into
//! them and checks the result as the parser's action would
//! ([temp.inst]/4, [class.base.init]). The section lists, per such
//! constructor record, the `:` location and each mem-initializer: its
//! mem-initializer-id (an identifier, or the type the parser resolved),
//! where it was written, whether it is a pack expansion (`B(args)...`,
//! [temp.variadic]/5) and its initializer (`( expression-list )` or a
//! braced-init-list). The expressions are records of the bodies section
//! (`Root::PatInit`). The importer installs the list on the constructor
//! once the bodies are built, before anything can instantiate it.
//!
//! Not carried (the constructor falls back): a mem-initializer whose
//! definition-time check failed (`failed_mem_inits`, already
//! diagnosed), and a braced-init-list with designators.

use kiln_ast::{DeclId, ExprId, QualType};
use kiln_source::Loc;

use super::body::{Built, Root};
use super::{Image, Locs, NONE, Payload, QRef, Rd, Res, W, Wr, Writer, bad};
use crate::Sema;
use crate::class::{MemInitId, MemInitInfo};
use crate::init::{InitItem, InitList, InitValue, Initializer};

/// The deepest braced-init-list nesting the decoder accepts.
const MAX_DEPTH: u32 = 64;

/// One mem-initializer-id.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum IdRec {
    /// An identifier (an index of the image's `pnames`).
    Ident(u32),
    /// A type the parser resolved.
    Type(QRef),
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
    Expr(u32),
    Paren(Vec<u32>, u32, u32),
    List(ListRec),
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct MemInitRec {
    id: IdRec,
    loc: u32,
    pack: bool,
    init: InitRec,
}

/// One constructor's list.
#[derive(Clone, PartialEq, Eq, Debug)]
struct PatInit {
    /// The constructor's record.
    rec: u32,
    /// The `:`.
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
    /// (a definition in a class template), if it has any.
    pub(super) fn pattern_mem_inits(&mut self, d: DeclId, i: u32) -> W<()> {
        let s = self.s;
        if s.inst.failed_mem_inits.contains(&d) {
            return Err("a mem-initializer that failed its definition-time check");
        }
        let Some((list, colon)) = s.inst.pattern_mem_inits.get(&d) else { return Ok(()) };
        let colon = self.loc(*colon)?;
        let mut inits = Vec::with_capacity(list.len());
        for m in list {
            let id = match m.id {
                MemInitId::Ident(sym) => {
                    let t = s.ast.symbols().as_str(sym).to_owned();
                    IdRec::Ident(Self::name_ix(&mut self.pnames, &mut self.img.pnames, &t))
                }
                MemInitId::Type(q) => IdRec::Type(self.qref(q)?),
            };
            let loc = self.loc(m.loc)?;
            let init = match &m.init {
                Initializer::Expr(e) => InitRec::Expr(self.pat_expr(*e)),
                Initializer::Paren(es, l, r) => {
                    let slots = es.iter().map(|&e| self.pat_expr(e)).collect();
                    InitRec::Paren(slots, self.loc(*l)?, self.loc(*r)?)
                }
                Initializer::List(l) => InitRec::List(self.pat_list(l)?),
            };
            inits.push(MemInitRec { id, loc, pack: m.pack_expansion, init });
        }
        self.img.patinits.ctors.push(PatInit { rec: i, colon, inits });
        Ok(())
    }

    fn pat_expr(&mut self, e: ExprId) -> u32 {
        let k = self.img.patinits.exprs.len() as u32;
        self.img.patinits.exprs.push(NONE);
        self.bs.roots.push(Root::PatInit(k, e));
        k
    }

    fn pat_list(&mut self, l: &InitList) -> W<ListRec> {
        let mut items = Vec::with_capacity(l.items.len());
        for it in &l.items {
            if !it.designator.is_empty() {
                return Err("a designated initializer in a mem-initializer");
            }
            items.push(match &it.value {
                InitValue::Expr(e) => ItemRec::Expr(self.pat_expr(*e)),
                InitValue::List(n) => ItemRec::List(self.pat_list(n)?),
            });
        }
        Ok(ListRec { items, lbrace: self.loc(l.lbrace)?, rbrace: self.loc(l.rbrace)? })
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
            }
            w.u32(m.loc);
            w.u8(u8::from(m.pack));
            match &m.init {
                InitRec::Expr(e) => (w.u8(0), w.u32(*e)).1,
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
        let m = r.count(11)?;
        let mut inits = Vec::with_capacity(m);
        for _ in 0..m {
            let id = match r.u8()? {
                0 => IdRec::Ident(r.u32()?),
                1 => IdRec::Type(r.q()?),
                _ => return bad("bad pattern mem-initializer"),
            };
            let loc = r.u32()?;
            let pack = match r.u8()? {
                0 => false,
                1 => true,
                _ => return bad("bad pattern mem-initializer"),
            };
            let init = match r.u8()? {
                0 => InitRec::Expr(r.u32()?),
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
            inits.push(MemInitRec { id, loc, pack, init });
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
            offs.push(m.loc);
            match &m.init {
                InitRec::Expr(_) => {}
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
                    IdRec::Ident(n) => (n as usize) < img.pnames.len(),
                    IdRec::Type(q) => qref_ok(q),
                };
                id_ok
                    && match &m.init {
                        InitRec::Expr(e) => slot_ok(*e),
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
    /// bodies), as the parser's action leaves them.
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
            let mut list = Vec::with_capacity(c.inits.len());
            for m in &c.inits {
                let id = match m.id {
                    IdRec::Ident(n) => MemInitId::Ident(self.intern(&img.pnames[n as usize])),
                    IdRec::Type(q) => MemInitId::Type(self.image_ty(img, tys, ids, q)),
                };
                let init = match &m.init {
                    InitRec::Expr(e) => Initializer::Expr(slot(*e)),
                    InitRec::Paren(es, l, r) => Initializer::Paren(
                        es.iter().map(|&e| slot(e)).collect(),
                        main.get(*l),
                        main.get(*r),
                    ),
                    InitRec::List(l) => Initializer::List(image_list(l, main, &slot)),
                };
                list.push(MemInitInfo { id, loc: main.get(m.loc), init, pack_expansion: m.pack });
            }
            let colon: Loc = main.get(c.colon);
            self.inst.pattern_mem_inits.insert(ids[c.rec as usize], (list, colon));
        }
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
