//! Class template specializations declared, then defined after other
//! declarations (spec/modules.md §11.11, "Forms carried by WP-1352"):
//! `template <> struct char_traits<char8_t>;` early, its definition (with
//! a base naming a template declared in between) later; a partial
//! specialization declared, then defined with a template-head of its own
//! (`template <size_t I, class... T> struct tuple_element<I, tuple<T...>>;`).
//!
//! Sema has one declaration for the specialization, so the writer's walk
//! in id order meets it at its first declaration, where the definition's
//! bases and members name records not yet written (a partial
//! specialization's definition, the parameters of its own head). The
//! specialization's listed record is then a declaration only, and the
//! definition is a second record, written when the walk reaches the
//! definition's first member: a `Record` with a definition whose `prev`
//! is the listed record. A partial specialization's definition record
//! also has its head in the [`LateHeads`] section (its parameters'
//! records just before it). The importer gives the specialization it made
//! for the listed record that definition, as the parser defines the
//! specialization it declared ([temp.expl.spec]/7, [temp.spec.partial]).

use kiln_ast::decl::DeclKind;
use kiln_ast::templates::SpecKind;
use kiln_ast::{DeclId, QualType, RawId as _};
use kiln_source::fx::FxHashMap;

use super::{
    DEP, DeclRec, Image, LEX_TU, Locs, NONE, Pass, Payload, Rd, Res, W, Wr, Writer, bad, classtempl,
};
use crate::Sema;

const REDECLARED_PARTIAL: &str = "a redeclared partial specialization";

/// The writer's state.
#[derive(Default)]
pub(super) struct LateDefs {
    /// Per first member of a definition still to write, the
    /// specialization.
    pending: FxHashMap<DeclId, DeclId>,
    /// Per specialization whose definition is written, its record (the
    /// context of its members).
    def_rec: FxHashMap<DeclId, u32>,
    /// The specialization whose definition record is being written.
    emitting: Option<DeclId>,
}

impl LateDefs {
    /// Drops the records from `n` on (a census rollback).
    pub(super) fn truncate(&mut self, n: u32) {
        self.def_rec.retain(|_, v| *v < n);
    }
}

/// A partial specialization definition's template-head.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct LateHead {
    /// The definition record.
    rec: u32,
    tloc: u32,
    langle: u32,
    rangle: u32,
    params: Vec<u32>,
}

/// The section of definition heads, ascending by record.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct LateHeads {
    heads: Vec<LateHead>,
}

impl LateHeads {
    fn get(&self, rec: u32) -> Option<&LateHead> {
        self.heads.binary_search_by_key(&rec, |h| h.rec).ok().map(|k| &self.heads[k])
    }

    /// Drops the entries of records from `n` on (a census rollback).
    pub(super) fn truncate(&mut self, n: u32) {
        self.heads.retain(|h| h.rec < n);
    }
}

impl Writer<'_, '_, '_> {
    /// The record members of class `c` name as their context: its
    /// definition record when that is a later one.
    pub(super) fn class_rec(&self, c: DeclId) -> Option<u32> {
        self.late.def_rec.get(&c).or_else(|| self.recs.get(&c)).copied()
    }

    /// Whether the definition of `d` is being written as its own record.
    pub(super) fn late_emitting(&self, d: DeclId) -> bool {
        self.late.emitting == Some(d)
    }

    /// Why partial specialization `d` (redeclared: declared, then
    /// defined) is not carried, or `None` when its definition is written
    /// as its own record.
    pub(super) fn redeclared_partial_why(&self, d: DeclId) -> Option<&'static str> {
        if !self.s.templ.partial_def_loc.contains_key(&d) || self.late_emitting(d) {
            return None;
        }
        match self.late_split(d) {
            Some(_) => None,
            None => Some(REDECLARED_PARTIAL),
        }
    }

    /// For explicit or partial class specialization `d`, defined: its
    /// first member, when a namespace-scope declaration of the purview
    /// (a partial specialization's own head included) lies between its
    /// first declaration and its definition (the definition is then
    /// written as its own record, there).
    pub(super) fn late_split(&self, d: DeclId) -> Option<DeclId> {
        let s = self.s;
        let ast = &s.ast;
        // One a declaration named before it was declared is visited where
        // replay declares it, its definition there (WP-1345, `members.rs`).
        if self.late.emitting == Some(d)
            || ast.definition(d) != Some(d)
            || self.deferred.contains(&d)
        {
            return None;
        }
        if ast.partial_spec_info(d).is_none() {
            let (_, si) = ast.spec_info(d)?;
            if si.kind != SpecKind::ExplicitSpecialization || si.from != d {
                return None;
            }
        } else if !s.templ.partial_def_head.contains_key(&d) {
            return None;
        }
        let ns = |x: Option<DeclId>| {
            x.is_some_and(|c| {
                matches!(
                    ast.decls.get(c).kind(),
                    DeclKind::TranslationUnit
                        | DeclKind::Namespace
                        | DeclKind::Export
                        | DeclKind::LinkageSpec
                )
            })
        };
        if !ns(ast.decls.get(d).ctx()) {
            return None;
        }
        // Not an implicit special member: sema declares one when it is
        // first needed, after declarations that are not between the
        // declaration and the definition (`template <> struct tag<0> {};`
        // whose default constructor a later `tag<1>()` declares; WP-1358).
        let first = ast.members(d).find(|&m| {
            !(ast.decls.get(m).has(kiln_ast::decl::DeclFlags::IMPLICIT)
                && s.special_kind_of(m).is_some())
        })?;
        let between = (d.raw() + 1..first.raw()).filter_map(DeclId::from_raw).any(|x| {
            let n = ast.decls.get(x);
            let o = ast.modules.owner(x);
            (o.unit == 0 && (o.purview || self.pch && self.pch_own(x)))
                && ns(n.ctx())
                && !n.has(kiln_ast::decl::DeclFlags::IMPLICIT)
                && !matches!(
                    n.kind(),
                    DeclKind::Namespace
                        | DeclKind::Export
                        | DeclKind::LinkageSpec
                        | DeclKind::ParmVar
                        | DeclKind::TranslationUnit
                )
                && !ast.spec_info(x).is_some_and(|(_, si)| {
                    matches!(si.kind, SpecKind::ImplicitInstantiation | SpecKind::Undeclared)
                })
        });
        between.then_some(first)
    }

    /// Notes that the definition of `d` is written before its first
    /// member `m`.
    pub(super) fn late_defer(&mut self, d: DeclId, m: DeclId) {
        self.late.pending.insert(m, d);
    }

    /// Before declaration `x` of the walk: the definition record of the
    /// specialization whose first member it is.
    pub(super) fn late_def_at(&mut self, x: DeclId, st: &mut Pass) -> W<()> {
        let Some(d) = self.late.pending.remove(&x) else { return Ok(()) };
        let Some(&i) = self.recs.get(&d) else { return Ok(()) };
        if self.census_dead(d) {
            return Ok(());
        }
        let before = self.marks();
        match self.late_def(d, i, st) {
            Ok(()) => Ok(()),
            Err(why) if self.census.is_some() => {
                self.rollback(before);
                self.census_fail(d, why, true);
                Ok(())
            }
            Err(why) => Err(why),
        }
    }

    fn late_def(&mut self, d: DeclId, i: u32, st: &mut Pass) -> W<()> {
        let s = self.s;
        let ast = &s.ast;
        // The specialization references the bases name, first.
        let bases: Vec<QualType> = ast
            .decls
            .record(d)
            .map(|r| ast.lists.get(r.bases).iter().map(|&g| ast.decls.base_spec(g).ty).collect())
            .unwrap_or_default();
        for q in bases {
            let _ = self.qref(q);
        }
        let mut h = self.head(d)?;
        let j = self.img.decls.len() as u32;
        // A partial specialization's definition: its own head, whose
        // parameters' records the walk wrote before its first member.
        let head = match s.templ.partial_def_head.get(&d) {
            None => None,
            Some(&list) => {
                let l = *ast.templates.param_list(list);
                let mut params = Vec::new();
                for q in ast.lists.iter::<DeclId>(l.params) {
                    params.push(*self.recs.get(&q).ok_or(super::tttp::PARM_OUTSIDE)?);
                }
                let at = s.templ.partial_def_loc.get(&d).copied().ok_or(REDECLARED_PARTIAL)?;
                h.loc = self.loc(at)?;
                Some(LateHead {
                    rec: j,
                    tloc: self.loc(l.template_loc)?,
                    langle: self.loc(l.l_angle)?,
                    rangle: self.loc(l.r_angle)?,
                    params,
                })
            }
        };
        self.late.emitting = Some(d);
        let p = self.class(d, j);
        self.late.emitting = None;
        let payload = p?;
        if self.img.decls.len() as u32 != j {
            return Err("a specialization named while its user is recorded");
        }
        // At namespace scope: no class is open around it.
        while let Some(top) = st.open.pop() {
            self.close_class(top);
        }
        let owner = self.img.decls[i as usize].owner;
        self.img.decls.push(DeclRec {
            flags: h.flags,
            owner,
            access: h.access,
            name: h.name,
            ctx: h.ctx,
            lexical: h.lexical,
            loc: h.loc,
            prev: i,
            payload,
        });
        if let Some(head) = head {
            self.img.lheads.heads.push(head);
        }
        self.late.def_rec.insert(d, j);
        st.open.push(j);
        if let Some(last) = self.s.ast.decls.record(d).and_then(|r| r.members.last) {
            st.open_last.insert(j, last);
        }
        Ok(())
    }
}

/// The listed explicit or partial specialization record whose definition
/// record `j` is.
pub(super) fn late_def_of(img: &Image, j: u32) -> Option<u32> {
    let d = img.decls.get(j as usize)?;
    match d.payload {
        Payload::Record { def: Some(_), .. }
            if d.prev != NONE
                && (classtempl::explicit_of(img, d.prev).is_some()
                    || classtempl::partial_of(img, d.prev).is_some()) =>
        {
            Some(d.prev)
        }
        _ => None,
    }
}

/// A record naming a listed class specialization as its previous
/// declaration is its definition: a class definition, after it, in its
/// context and with its name and flags, the listed record a declaration
/// only. A partial specialization's definition has a head of its own,
/// whose parameters it claims (and itself), as a listed partial
/// specialization does (`classtempl::validate_class_specs`, which runs
/// first).
pub(super) fn validate_late_defs(img: &Image, claimed: &mut [bool]) -> Res<()> {
    let hs = &img.lheads.heads;
    if !hs.windows(2).all(|w| w[0].rec < w[1].rec) {
        return bad("bad definition of a partial specialization");
    }
    let mut seen = 0;
    for (j, d) in img.decls.iter().enumerate() {
        if d.prev == NONE {
            continue;
        }
        let partial = classtempl::partial_of(img, d.prev);
        if classtempl::explicit_of(img, d.prev).is_none() && partial.is_none() {
            continue;
        }
        let p = &img.decls[d.prev as usize];
        let ok = (d.prev as usize) < j
            && matches!(d.payload, Payload::Record { def: Some(_), .. })
            && matches!(p.payload, Payload::Record { def: None, .. })
            && d.name == p.name
            && d.ctx == p.ctx
            && d.lexical == p.lexical
            && d.flags == p.flags
            && d.access == p.access
            && d.owner == p.owner;
        if !ok {
            return bad("bad definition of a class specialization");
        }
        let head = img.lheads.get(j as u32);
        let Some(t) = partial else {
            if head.is_some() {
                return bad("bad definition of a class specialization");
            }
            continue;
        };
        let (Some(h), Some(Payload::Template { depth, .. })) =
            (head, img.decls.get(t as usize).map(|r| &r.payload))
        else {
            return bad("bad definition of a partial specialization");
        };
        seen += 1;
        let scope = match d.lexical {
            NONE => d.ctx,
            LEX_TU => NONE,
            l => l,
        };
        for (k, &q) in h.params.iter().enumerate() {
            let ok = (q as usize) < j
                && q > d.prev
                && !claimed[q as usize]
                && img.decls.get(q as usize).is_some_and(|x| {
                    x.ctx == scope
                        && x.flags & DEP != 0
                        && match x.payload {
                            Payload::Ttp { depth: dd, index, .. }
                            | Payload::Nttp { depth: dd, index, .. } => {
                                dd == *depth && usize::from(index) == k
                            }
                            Payload::Tttp(ref t) => t.depth == *depth && usize::from(t.index) == k,
                            _ => false,
                        }
                });
            if !ok {
                return bad("bad template parameter list");
            }
            claimed[q as usize] = true;
        }
        if d.flags & DEP == 0 || claimed[j] {
            return bad("bad definition of a partial specialization");
        }
        claimed[j] = true;
    }
    if seen != hs.len() {
        return bad("bad definition of a partial specialization");
    }
    Ok(())
}

pub(super) fn encode_late_heads(w: &mut Wr, s: &LateHeads) {
    w.len(s.heads.len());
    for h in &s.heads {
        for x in [h.rec, h.tloc, h.langle, h.rangle] {
            w.u32(x);
        }
        w.len(h.params.len());
        for &p in &h.params {
            w.u32(p);
        }
    }
}

pub(super) fn decode_late_heads(r: &mut Rd) -> Res<LateHeads> {
    let n = r.count(20)?;
    let mut heads = Vec::with_capacity(n);
    for _ in 0..n {
        let (rec, tloc, langle, rangle) = (r.u32()?, r.u32()?, r.u32()?, r.u32()?);
        let k = r.count(4)?;
        let mut params = Vec::with_capacity(k);
        for _ in 0..k {
            params.push(r.u32()?);
        }
        heads.push(LateHead { rec, tloc, langle, rangle, params });
    }
    Ok(LateHeads { heads })
}

/// The section's locations, for the decoder's offset check.
pub(super) fn late_head_locs(img: &Image, offs: &mut Vec<u32>) {
    for h in &img.lheads.heads {
        offs.extend([h.tloc, h.langle, h.rangle]);
    }
}

/// The importer's side.
impl Sema<'_, '_> {
    /// Definition record `i` of class specialization `c` (declared by an
    /// earlier record): a partial specialization's definition head is
    /// built as the parser builds it, and noted where the definition is
    /// (`partial_def_loc`, the redefinition note's location).
    pub(super) fn image_late_def(
        &mut self,
        img: &Image,
        ids: &[DeclId],
        main: &Locs,
        i: u32,
        c: DeclId,
        at: kiln_source::Loc,
    ) -> DeclId {
        let Some(h) = img.lheads.get(i) else { return c };
        let ps: Vec<DeclId> = h.params.iter().map(|&p| ids[p as usize]).collect();
        let (t, l, r) = (main.get(h.tloc), main.get(h.langle), main.get(h.rangle));
        let depth = match img.decls[i as usize].prev {
            p if p != NONE => classtempl::partial_of(img, p)
                .and_then(|t| match img.decls[t as usize].payload {
                    Payload::Template { depth, .. } => Some(depth),
                    _ => None,
                })
                .unwrap_or(0),
            _ => 0,
        };
        let list = self.builder().template_param_list(t, l, r, depth, &ps, None);
        self.templ.partial_def_loc.insert(c, at);
        self.templ.partial_def_head.insert(c, list);
        c
    }
}

#[cfg(test)]
mod tests {
    use super::super::selfbase::tests::sample as class_template_sample;
    use super::super::{QRef, RecDef, decode, encode};
    use super::*;
    use kiln_ast::types::BuiltinType;

    /// The WP-1344 class template sample plus `template <> struct
    /// Ch<int>;` (record `k`) and, two records on, its definition
    /// (`k + 2`), a namespace-scope class between them.
    fn sample() -> (Image, usize) {
        let (mut img, p) = class_template_sample();
        let tmpl = p as u32 + 1;
        let int = QRef { ty: BuiltinType::Int as u32, quals: 0 };
        let k = img.decls.len();
        let named = |name, payload| DeclRec {
            flags: 0,
            owner: 3,
            access: 0,
            name,
            ctx: 1,
            lexical: NONE,
            loc: 440,
            prev: NONE,
            payload,
        };
        let cname = img.decls[p].name;
        let other = img.names.len() as u32;
        img.names.push("Other".into());
        let def = |end| RecDef { bits: 0, end, key: NONE, bases: vec![], friends: vec![] };
        img.decls.push(named(cname, Payload::Record { tag: 0, def: None }));
        img.decls.push(named(other, Payload::Record { tag: 0, def: Some(def(k as u32 + 2)) }));
        let mut d = named(cname, Payload::Record { tag: 0, def: Some(def(k as u32 + 3)) });
        d.prev = k as u32;
        img.decls.push(d);
        img.cspecs.push_explicit(k as u32, tmpl, &[int]);
        (img, k)
    }

    /// spec/modules.md §11.11 "Forms carried by WP-1352": an explicit
    /// specialization's later definition record round-trips.
    #[test]
    fn late_definitions_round_trip() {
        let (img, k) = sample();
        assert_eq!(late_def_of(&img, k as u32 + 2), Some(k as u32));
        assert_eq!(decode(&encode(&img)), Ok(img));
    }

    /// A definition record of another name, one that is no definition,
    /// one whose listed record is itself a definition, or a template-head
    /// on an explicit specialization's definition, is an error.
    #[test]
    fn rejects_corrupt_late_definitions() {
        type Corrupt = (&'static str, fn(&mut Image, usize));
        let corrupt: [Corrupt; 4] = [
            ("a head on an explicit specialization's definition", |img, k| {
                let rec = k as u32 + 2;
                let head = LateHead { rec, tloc: 390, langle: 398, rangle: 404, params: vec![] };
                img.lheads.heads.push(head);
            }),
            ("another name", |img, k| img.decls[k + 2].name = img.decls[k + 1].name),
            ("not a definition", |img, k| {
                img.decls[k + 2].payload = Payload::Record { tag: 0, def: None };
            }),
            ("the listed record defined too", |img, k| {
                img.decls[k].payload = Payload::Record {
                    tag: 0,
                    def: Some(RecDef {
                        bits: 0,
                        end: k as u32 + 1,
                        key: NONE,
                        bases: vec![],
                        friends: vec![],
                    }),
                };
            }),
        ];
        for (what, f) in corrupt {
            let (mut img, k) = sample();
            f(&mut img, k);
            assert!(decode(&encode(&img)).is_err(), "{what}");
        }
    }
}
