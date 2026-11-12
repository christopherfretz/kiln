//! Template template parameters and template arguments that name
//! templates in the declaration image (spec/modules.md §11.11, "Forms
//! carried by WP-1342").
//!
//! A template template parameter ([temp.param]/1, `template <class...>
//! class TT`) is a record of kind [`super::K_TTTP`] after the records of
//! its own parameters (which the parser declares first, one level
//! deeper): its depth, index, pack and key bits, its kind (a type,
//! variable or concept template parameter, P2841), its first token, its
//! template-head (the `template`, `<` and `>` locations, depth and
//! parameter records) and its default argument, a template.
//!
//! A template argument naming a template ([temp.arg.template]) is
//! [`ArgRec::Template`]: the record of a carried template or template
//! template parameter, and whether it is a pack expansion `TT...` (with
//! its expansion count). A specialization type `TT<args>` whose template
//! is a template template parameter is a dependent [`super::TyRec::Tst`]
//! naming the parameter's record. The importer names each with
//! `template_name_for`, as the parser does, so canonical types and
//! printed names are the replay's. A dependent template name
//! `nns::template name` as an argument is [`ArgRec::DepTemplate`]: a
//! type record `nns::template name<>` (a [`DepTy::Tmpl`] without
//! arguments) that carries the qualifier and the name, from which the
//! importer takes them back.
//!
//! Not carried (the declaration falls back with the named reason): a
//! template template parameter written without `class` or `typename`
//! (clang 22 drops its uses), one whose own head has a requires-clause,
//! a constrained one, and a template argument naming a template through
//! a non-dependent qualifier, a using-declaration or a substitution.

use kiln_ast::decl::DeclKind;
use kiln_ast::templates::TtpKind;
use kiln_ast::types::{ArgView, TemplateArg, TemplateName, TypeView};
use kiln_ast::{DeclId, TemplateNameId};

use super::deptypes::DepTy;
use super::{ArgRec, Image, NONE, Payload, Rd, Res, TyRec, W, Wr, Writer, bad};
use crate::Sema;

/// A template template parameter record's payload.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct TttpRec {
    pub(super) depth: u16,
    pub(super) index: u16,
    pub(super) pack: bool,
    typename_kw: bool,
    /// 0 a type, 1 a variable, 2 a concept template parameter.
    pub(super) kind: u8,
    /// Its first token when that is not its name, or [`NONE`].
    begin: u32,
    tloc: u32,
    langle: u32,
    rangle: u32,
    /// The depth of its own template-head.
    list_depth: u16,
    /// Its own parameters' records.
    pub(super) params: Vec<u32>,
    /// Its default argument (an [`ArgRec::Template`]).
    default: Option<ArgRec>,
}

/// The reason for a template parameter whose record the image lacks:
/// the parameter's own refusal is counted at the parameter.
pub(super) const PARM_OUTSIDE: &str = "a template parameter outside the image";

/// The census form of a template-name argument the image does not carry.
const TNAME: &str = "a template argument naming a template the image does not carry";

impl Writer<'_, '_, '_> {
    /// The record of template template parameter `d`.
    pub(super) fn tttp(&mut self, d: DeclId) -> W<Payload> {
        let s = self.s;
        let ast = &s.ast;
        let p = *ast.template_parm(d).ok_or("a malformed template parameter")?;
        let list = p.params.ok_or("a malformed template parameter")?;
        if s.templ.keyless_ttps.contains(&d) {
            return Err("a template template parameter without class or typename");
        }
        if p.constraint.is_some() || ast.templates.ttp_owner.contains(d) {
            return Err("a constrained template template parameter");
        }
        let l = *ast.templates.param_list(list);
        if l.requires.is_some() {
            return Err("a template template parameter with a requires-clause");
        }
        let mut params = Vec::new();
        for q in ast.lists.iter::<DeclId>(l.params) {
            params.push(*self.recs.get(&q).ok_or(PARM_OUTSIDE)?);
        }
        let default = match ast.templates.defaults.get(d).map(TemplateArg::view) {
            None => None,
            Some(ArgView::Template(n)) => Some(self.tname_arg(n, 0)?),
            Some(_) => return Err("a default template argument"),
        };
        let begin = match ast.decls.cold.begin_loc.get(d) {
            Some(&b) => self.loc(b)?,
            None => NONE,
        };
        let kind = match p.ttp_kind {
            TtpKind::Type => 0,
            TtpKind::Variable => 1,
            TtpKind::Concept => 2,
        };
        Ok(Payload::Tttp(Box::new(TttpRec {
            depth: p.depth,
            index: p.index,
            pack: p.pack,
            typename_kw: p.typename_kw,
            kind,
            begin,
            tloc: self.loc(l.template_loc)?,
            langle: self.loc(l.l_angle)?,
            rangle: self.loc(l.r_angle)?,
            list_depth: l.depth,
            params,
            default,
        })))
    }

    /// Template argument `n` (`pack`: 0 plain, 1 an expansion of unknown
    /// length, `k + 2` one of `k` elements): a carried template or
    /// template template parameter named directly.
    pub(super) fn tname_arg(&mut self, n: TemplateNameId, pack: u32) -> W<ArgRec> {
        let ast = &self.s.ast;
        let d = match ast.types.template_name_view(n) {
            TemplateName::Template(d) => ast.first(d),
            TemplateName::TemplateTemplateParm { decl, .. } => decl,
            // `nns::template name`: carried as the type `nns::template
            // name<>`, from which the importer takes it back.
            TemplateName::DependentIdentifier { nns, name } => {
                let steps = self.nns_steps(nns)?;
                let name = self.sym_ix(name);
                let kw = kiln_ast::types::TagKeyword::Typename as u8;
                let rec = TyRec::Dep(DepTy::Tmpl { kw, steps, name, args: Vec::new() });
                let ty = super::BUILTINS + 1 + self.img.types.len() as u32;
                self.img.types.push(rec);
                return Ok(ArgRec::DepTemplate(ty, pack));
            }
            _ => return Err(TNAME),
        };
        let rec = *self.recs.get(&d).ok_or(TNAME)?;
        if !tname_rec(&self.img, rec) {
            return Err(TNAME);
        }
        Ok(ArgRec::Template(rec, pack))
    }

    /// The record of the template template parameter a specialization
    /// type `TT<args>` names (WP-1342), `None` for another name.
    pub(super) fn tst_ttp(&self, n: TemplateNameId) -> W<Option<u32>> {
        let ast = &self.s.ast;
        let TemplateName::TemplateTemplateParm { decl, .. } = ast.types.template_name_view(n)
        else {
            return Ok(None);
        };
        match self.recs.get(&decl) {
            Some(&r) if matches!(self.img.decls[r as usize].payload, Payload::Tttp(_)) => {
                Ok(Some(r))
            }
            _ => Err(PARM_OUTSIDE),
        }
    }
}

/// Whether record `rec` may be named by a template argument: a class,
/// alias or variable template, a concept, or a template template
/// parameter.
pub(super) fn tname_rec(img: &Image, rec: u32) -> bool {
    matches!(
        img.decls.get(rec as usize).map(|r| &r.payload),
        Some(Payload::Template { kind: 1..=4, .. } | Payload::Tttp(_))
    )
}

pub(super) fn encode_tttp(w: &mut Wr, t: &TttpRec) {
    w.u16(t.depth);
    w.u16(t.index);
    w.u8(u8::from(t.pack) | u8::from(t.typename_kw) << 1 | t.kind << 2);
    for v in [t.begin, t.tloc, t.langle, t.rangle] {
        w.u32(v);
    }
    w.u16(t.list_depth);
    w.len(t.params.len());
    for &p in &t.params {
        w.u32(p);
    }
    if let Some(ArgRec::Template(r, k)) = t.default {
        w.u32(r);
        w.u32(k);
    } else {
        w.u32(NONE);
        w.u32(0);
    }
}

pub(super) fn decode_tttp(r: &mut Rd<'_>) -> Res<Payload> {
    let (depth, index, bits) = (r.u16()?, r.u16()?, r.u8()?);
    if bits >> 2 > 2 {
        return bad("bad flag byte");
    }
    let (begin, tloc, langle, rangle) = (r.u32()?, r.u32()?, r.u32()?, r.u32()?);
    let list_depth = r.u16()?;
    let n = r.count(4)?;
    let mut params = Vec::with_capacity(n);
    for _ in 0..n {
        params.push(r.u32()?);
    }
    let (d, k) = (r.u32()?, r.u32()?);
    let default = match (d, k) {
        (NONE, 0) => None,
        (NONE, _) => return bad("bad default template argument"),
        (d, 0) => Some(ArgRec::Template(d, 0)),
        _ => return bad("bad default template argument"),
    };
    Ok(Payload::Tttp(Box::new(TttpRec {
        depth,
        index,
        pack: bits & 1 != 0,
        typename_kw: bits & 2 != 0,
        kind: bits >> 2,
        begin,
        tloc,
        langle,
        rangle,
        list_depth,
        params,
        default,
    })))
}

/// The record's locations, for the decoder's offset check.
pub(super) fn tttp_locs(t: &TttpRec, offs: &mut Vec<u32>) {
    offs.extend([t.begin, t.tloc, t.langle, t.rangle]);
}

/// Checks template template parameter record `i` and claims its own
/// parameters: each an earlier parameter record one level deeper, in
/// order; its default an earlier template.
pub(super) fn validate_tttp(img: &Image, i: usize, t: &TttpRec, claimed: &mut [bool]) -> Res<()> {
    for (k, &p) in t.params.iter().enumerate() {
        let ok = (p as usize) < i
            && !claimed[p as usize]
            && img.decls.get(p as usize).is_some_and(|x| {
                x.ctx == img.decls[i].ctx
                    && match &x.payload {
                        Payload::Ttp { depth, index, .. } | Payload::Nttp { depth, index, .. } => {
                            *depth == t.list_depth && usize::from(*index) == k
                        }
                        Payload::Tttp(x) => x.depth == t.list_depth && usize::from(x.index) == k,
                        _ => false,
                    }
            });
        if !ok {
            return bad("bad template parameter list");
        }
        claimed[p as usize] = true;
    }
    if let Some(ArgRec::Template(d, _)) = t.default
        && ((d as usize) >= i || !tname_rec(img, d))
    {
        return bad("bad default template argument");
    }
    Ok(())
}

/// Checks the template-name arguments of `args`, named by record or type
/// `at`: earlier templates or template template parameters. The highest
/// record (plus one) named.
pub(super) fn validate_tname_args(img: &Image, args: &[ArgRec], at: usize) -> Res<usize> {
    let mut hi = 0;
    for a in args {
        if let ArgRec::Template(d, _) = *a {
            if (d as usize) >= at || !tname_rec(img, d) {
                return bad("bad template argument");
            }
            hi = hi.max(d as usize + 1);
        }
        if let ArgRec::DepTemplate(t, _) = *a
            && !dep_tname_ty(img, t)
        {
            return bad("bad template argument");
        }
    }
    Ok(hi)
}

/// The importer's side.
impl Sema<'_, '_> {
    /// Builds template template parameter record `t` (header `h`, its
    /// parameters among `ids`).
    pub(super) fn image_tttp(
        &mut self,
        h: kiln_ast::DeclHead,
        t: &TttpRec,
        ids: &[DeclId],
        lo: &dyn Fn(&Self, u32) -> kiln_source::Loc,
    ) -> DeclId {
        let ps: Vec<DeclId> = t.params.iter().map(|&p| ids[p as usize]).collect();
        let (tl, l, r) = (lo(self, t.tloc), lo(self, t.langle), lo(self, t.rangle));
        let list = self.builder().template_param_list(tl, l, r, t.list_depth, &ps, None);
        let d =
            self.builder().template_template_parm(h, list, t.depth, t.index, t.pack, t.typename_kw);
        match t.kind {
            1 => self.builder().set_ttp_kind(d, TtpKind::Variable),
            2 => self.builder().set_ttp_kind(d, TtpKind::Concept),
            _ => {}
        }
        if t.begin != NONE {
            let _ = self.ast.decls.cold.begin_loc.insert(d, lo(self, t.begin));
        }
        if let Some(ArgRec::Template(x, _)) = t.default
            && let Some(&a) = ids.get(x as usize)
        {
            let n = self.template_name_for(a);
            let _ = self.builder().set_template_default(d, TemplateArg::template(n));
        }
        d
    }

    /// Image template argument `a` (whose type is `ty`), naming records
    /// `ids`: [`Sema::image_arg`], and a template name (WP-1342).
    pub(super) fn image_targ(
        &mut self,
        ids: &[DeclId],
        ty: kiln_ast::QualType,
        a: &ArgRec,
    ) -> TemplateArg {
        let (n, pack) = match *a {
            ArgRec::Template(x, pack) => {
                let Some(&d) = ids.get(x as usize) else { return TemplateArg::NULL };
                (self.template_name_for(d), pack)
            }
            ArgRec::DepTemplate(_, pack) => {
                let TypeView::DependentTemplateSpecialization { nns, name, .. } =
                    self.ast.types.view(ty.ty())
                else {
                    return TemplateArg::NULL;
                };
                let n = TemplateName::DependentIdentifier { nns, name };
                (self.ast.types.template_name(n), pack)
            }
            _ => return self.image_arg(ty, a),
        };
        match pack {
            0 => TemplateArg::template(n),
            1 => TemplateArg::template_expansion(n, None),
            k => TemplateArg::template_expansion(n, Some(k - 2)),
        }
    }

    /// The template a specialization type's record `d` names: a template
    /// template parameter as itself (WP-1342), a template as its first
    /// declaration.
    pub(super) fn image_tst_name(&mut self, d: DeclId) -> TemplateNameId {
        if self.ast.decls.get(d).kind() == DeclKind::TemplateTemplateParm {
            return self.template_name_for(d);
        }
        let first = self.ast.first(d);
        self.ast.types.template_name_of(first)
    }
}

/// Whether type reference `t` is the carrier of a dependent template
/// name: `nns::template name<>`.
pub(super) fn dep_tname_ty(img: &Image, t: u32) -> bool {
    t.checked_sub(super::BUILTINS + 1)
        .and_then(|j| img.types.get(j as usize))
        .is_some_and(|r| matches!(r, TyRec::Dep(DepTy::Tmpl { args, .. }) if args.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::super::tests::class_template_sample;
    use super::super::{BUILTINS, DEP, DeclRec, QRef, RecDef, decode, encode};
    use super::*;
    use kiln_ast::types::BuiltinType;

    /// `class_template_sample` (record 16 is class template `Box`) plus
    /// `template <template <class> class W = Box> struct Wrap { W<int> w; };`
    /// and the type `W<int>`; the template template parameter's record.
    fn tttp_sample() -> (Image, usize) {
        let q = |ty| QRef { ty, quals: 0 };
        let int = BuiltinType::Int as u32;
        let mut img = class_template_sample();
        let name = img.names.len() as u32;
        img.names.extend(["Wrap", "w"].map(String::from));
        let pname = img.pnames.len() as u32;
        img.pnames.extend(["U", "W"].map(String::from));
        let i = img.decls.len() as u32;
        let head = |flags, name, ctx, loc, payload| DeclRec {
            flags,
            owner: 3,
            access: 0,
            name,
            ctx,
            lexical: NONE,
            loc,
            prev: NONE,
            payload,
        };
        let t = BUILTINS + 1 + img.types.len() as u32;
        img.types.push(TyRec::Tst {
            template: i + 1,
            args: vec![ArgRec::Type(q(int))],
            under: None,
        });
        let w = TttpRec {
            depth: 0,
            index: 0,
            pack: false,
            typename_kw: false,
            kind: 0,
            begin: 480,
            tloc: 480,
            langle: 488,
            rangle: 496,
            list_depth: 1,
            params: vec![i],
            default: Some(ArgRec::Template(16, 0)),
        };
        img.decls.extend([
            head(
                DEP,
                pname,
                1,
                494,
                Payload::Ttp {
                    depth: 1,
                    index: 0,
                    typename_kw: false,
                    pack: false,
                    begin: 489,
                    default: None,
                },
            ),
            head(DEP, pname + 1, 1, 504, Payload::Tttp(Box::new(w))),
            head(
                DEP,
                name,
                1,
                520,
                Payload::Record {
                    tag: 0,
                    def: Some(RecDef {
                        bits: 0,
                        end: i + 5,
                        key: NONE,
                        bases: vec![],
                        friends: vec![],
                    }),
                },
            ),
            head(
                0,
                name,
                1,
                520,
                Payload::Template {
                    kind: 1,
                    pattern: i + 2,
                    tloc: 476,
                    using: NONE,
                    langle: 478,
                    rangle: 512,
                    depth: 0,
                    params: vec![i + 1],
                },
            ),
            head(
                DEP,
                name + 1,
                i + 2,
                530,
                Payload::Field { ty: q(t), flags: 0, width: NONE, init: NONE },
            ),
        ]);
        (img, i as usize + 1)
    }

    /// spec/modules.md §11.11 "Forms carried by WP-1342": a template
    /// template parameter with its own parameter and a default, and a
    /// specialization type naming it, round-trip.
    #[test]
    fn template_template_parameters_round_trip() {
        let (img, _) = tttp_sample();
        assert_eq!(decode(&encode(&img)), Ok(img));
    }

    /// A template template parameter whose own parameters or default
    /// are not what it claims is an error.
    #[test]
    fn rejects_corrupt_template_template_parameters() {
        type Corrupt = (&'static str, fn(&mut TttpRec, usize));
        let corrupt: [Corrupt; 4] = [
            ("parameter at the wrong depth", |t, _| t.list_depth = 0),
            ("parameter not before it", |t, i| t.params = vec![i as u32]),
            ("default naming a later record", |t, i| {
                t.default = Some(ArgRec::Template(i as u32 + 2, 0));
            }),
            ("default naming a non-template", |t, _| t.default = Some(ArgRec::Template(17, 0))),
        ];
        for (what, f) in corrupt {
            let (mut img, i) = tttp_sample();
            let Payload::Tttp(t) = &mut img.decls[i].payload else { unreachable!() };
            f(t, i);
            assert!(decode(&encode(&img)).is_err(), "{what}");
        }
    }
}
