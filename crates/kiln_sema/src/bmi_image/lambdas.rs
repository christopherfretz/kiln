//! Lambda expressions, local classes and local enumerations in the
//! declaration image (spec/modules.md §11.11, "Forms carried by
//! WP-1350").
//!
//! A *lazy tag* is a closure type ([expr.prim.lambda.closure]) or a class
//! or enumeration whose context is a function ([class.local],
//! [dcl.enum]). Like a local variable it gets its records only when
//! something carried names it: its type, one of its members, its
//! lambda-expression or its declaration statement. Then the tag and
//! everything declared in it (members, nested classes, a generic call
//! operator's template and its parameters, the implicit members sema
//! declared) are written at once, in declaration order, as the collect
//! pass writes a class at namespace scope, so a class definition stays
//! one contiguous run of records. Nothing else is recorded meanwhile: a
//! tag named while another is being written falls back.
//!
//! A closure type's record is a class definition with the
//! `DefinitionBits::LAMBDA` bit; its lambda's side record (captures,
//! introducer, mangling number and context) is a record of the
//! [`Lambdas`] section, which a `LambdaExpr` names ([`F::LB`]) and whose
//! capture initializers are that expression's operands. The conversion
//! function to pointer to function and the static invoker are ordinary
//! implicit member records. The closure type's identity is the replay's:
//! the image carries the mangling number and context sema gave it, so
//! `Ul..E[n]_` names agree; a closure without a number (`$_N`) is named
//! when it is first mangled, as in replay.
//!
//! [`F::LB`]: super::body::F::LB

use kiln_ast::decl::{DeclFlags, DeclKind, DefinitionBits};
use kiln_ast::ids::LambdaId;
use kiln_ast::lambda::{CaptureDefault, CaptureKind, LambdaCapture, LambdaData};
use kiln_ast::{DeclHead, DeclId, ExprId, ListId, RawId};
use kiln_source::fx::FxHashSet;

use super::body::{Built, Dc, PARAM};
use super::{DECL_FLAGS, DeclRec, Image, NONE, Pass, Payload, Rd, Res, W, Wr, Writer, bad};
use crate::Sema;

/// The closure bits a class record may carry besides `DEF_BITS`.
pub(super) const CLOSURE_BITS: u64 =
    DefinitionBits::LAMBDA.bits() | DefinitionBits::LAMBDA_CAPTURES.bits();

/// [`CapRec::flags`]: captured through the capture-default.
const CAP_IMPLICIT: u8 = 1;
/// [`CapRec::flags`]: an init-capture.
const CAP_INIT: u8 = 2;
/// [`LambdaRec::flags`]: a parameter-declaration-clause was written.
const L_PARAMS: u8 = 1;
/// [`LambdaRec::flags`]: a trailing return type was written.
const L_RESULT: u8 = 2;
/// [`LambdaRec::flags`]: `mutable`.
const L_MUTABLE: u8 = 4;
/// [`LambdaRec::flags`]: the capture initializers are set (the closure
/// is complete).
const L_INITS: u8 = 8;

/// One capture, in closure member order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct CapRec {
    /// `CaptureKind`: 0 `this`, 1 `*this`, 2 by copy, 3 by reference.
    kind: u8,
    /// The captured entity (a declaration word: a variable record, a
    /// parameter `PARAM | k`, a structured binding), an init-capture's
    /// own variable, or [`NONE`] for `this`.
    var: u32,
    /// The closure's member that holds it, or [`NONE`].
    field: u32,
    loc: u32,
    ellipsis: u32,
    flags: u8,
}

/// A lambda's side record.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct LambdaRec {
    /// The closure type's record.
    pub(super) closure: u32,
    /// The function call operator's record (a generic lambda's: its
    /// template's pattern), or [`NONE`].
    call_op: u32,
    /// `CaptureDefault`: 0 none, 1 `=`, 2 `&`.
    default: u8,
    default_loc: u32,
    lbracket: u32,
    rbracket: u32,
    flags: u8,
    /// The mangling number (0: none) and context declaration word.
    number: u32,
    context: u32,
    caps: Vec<CapRec>,
    /// The unexpanded parameter packs the lambda's body names
    /// (declaration words), which sema keeps beside the lambda.
    packs: Vec<u32>,
    /// The capture initializers: expressions of the bodies, operands of
    /// the lambda-expression that names this record.
    inits: Vec<u32>,
}

/// The section of the image's lambdas.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct Lambdas {
    pub(super) recs: Vec<LambdaRec>,
}

impl Lambdas {
    /// Drops the records of closures from record `n` on (a census
    /// rollback).
    pub(super) fn truncate(&mut self, n: u32) {
        self.recs.retain(|r| r.closure < n);
    }
}

/// The writer's state.
#[derive(Default)]
pub(super) struct LamState {
    /// The tags being written, innermost last.
    pub(super) writing: Vec<DeclId>,
    /// The template parameters of the tag being written's member
    /// templates ([`Writer::tag_decls`]).
    heads: FxHashSet<DeclId>,
    /// Every generic lambda's template parameters
    /// ([`Writer::generic_heads`]).
    generic: std::cell::OnceCell<FxHashSet<DeclId>>,
    /// The record ranges of the tags written.
    ranges: Vec<(u32, u32)>,
    /// Lambda records whose mangling context was not yet recorded.
    pending: Vec<(u32, DeclId)>,
    /// The tags that could not be written, and why.
    failed: kiln_source::fx::FxHashMap<DeclId, &'static str>,
    /// Lambda → section record.
    index: kiln_source::fx::FxHashMap<LambdaId, u32>,
}

fn cap_kind(k: CaptureKind) -> u8 {
    match k {
        CaptureKind::This => 0,
        CaptureKind::StarThis => 1,
        CaptureKind::ByCopy => 2,
        CaptureKind::ByRef => 3,
    }
}

fn kind_of_cap(k: u8) -> CaptureKind {
    match k {
        0 => CaptureKind::This,
        1 => CaptureKind::StarThis,
        2 => CaptureKind::ByCopy,
        _ => CaptureKind::ByRef,
    }
}

impl Writer<'_, '_, '_> {
    /// Whether `d` is a lazy tag: a closure type or a local class or
    /// enumeration.
    fn is_tag(&self, d: DeclId) -> bool {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        match n.kind() {
            DeclKind::CXXRecord if ast.decls.record(d).is_some_and(|r| r.lambda.is_some()) => true,
            DeclKind::CXXRecord | DeclKind::Enum => {
                n.ctx().is_some_and(|c| ast.decls.function(c).is_some())
            }
            _ => false,
        }
    }

    /// The innermost lazy tag `d` is or is declared in (with no function
    /// between them): `None` for a local of a function.
    pub(super) fn tag_of(&self, d: DeclId) -> Option<DeclId> {
        let decls = &self.s.ast.decls;
        let mut cur = d;
        for step in 0..4096 {
            if step > 0 && decls.function(cur).is_some() {
                return None;
            }
            if self.is_tag(cur) {
                return Some(cur);
            }
            cur = decls.get(cur).ctx()?;
        }
        None
    }

    /// Whether the collect pass leaves `d` to the tag it is in: written
    /// when the tag is first named.
    pub(super) fn lazy_skip(&self, d: DeclId) -> bool {
        if matches!(
            self.s.ast.decls.get(d).kind(),
            DeclKind::TemplateTypeParm
                | DeclKind::NonTypeTemplateParm
                | DeclKind::TemplateTemplateParm
        ) && self.generic_heads().contains(&d)
        {
            // A generic lambda's template parameter, declared in the
            // lambda's scope: written with its closure type.
            return !self.bs.lam.heads.contains(&d);
        }
        self.tag_of(d).is_some_and(|t| !self.bs.lam.writing.contains(&t))
    }

    /// Every generic lambda's template parameters.
    fn generic_heads(&self) -> &FxHashSet<DeclId> {
        self.bs.lam.generic.get_or_init(|| {
            let ast = &self.s.ast;
            let mut out = FxHashSet::default();
            for k in 1..=ast.exprs.lambdas.len() as u32 {
                let Some(op) = ast.exprs.lambdas.get(LambdaId::new(k)).call_op else { continue };
                let Some(info) = ast.described_template(op).and_then(|t| ast.template_info(t))
                else {
                    continue;
                };
                let ps = ast.templates.param_list(info.params).params;
                out.extend(ast.lists.iter::<DeclId>(ps));
            }
            out
        })
    }

    /// Whether `d` is a template parameter of the tag being written's
    /// member template, declared in the scope the tag is in.
    pub(super) fn lazy_head(&self, d: DeclId) -> bool {
        self.bs.lam.heads.contains(&d)
    }

    /// The record of the tag being written (a generic lambda's template
    /// parameter's context, [`Writer::lazy_head`]).
    pub(super) fn writing_rec(&self) -> W<u32> {
        let t = self.bs.lam.writing.last().ok_or("a malformed lambda")?;
        self.recs.get(t).copied().ok_or("a malformed lambda")
    }

    /// Whether `d` is in the tag being written.
    pub(super) fn in_lazy_write(&self, d: DeclId) -> bool {
        self.bs.lam.heads.contains(&d)
            || self.tag_of(d).is_some_and(|t| self.bs.lam.writing.contains(&t))
    }

    /// Whether `d` may carry `IMPLICIT` in its record: a closure type, or
    /// its conversion function or static invoker (or their templates).
    pub(super) fn lazy_implicit(&self, d: DeclId) -> bool {
        let ast = &self.s.ast;
        if ast.closure_lambda(d).is_some() {
            return true;
        }
        let f = match ast.decls.get(d).kind() {
            DeclKind::FunctionTemplate => match ast.template_info(d).and_then(|i| i.pattern) {
                Some(p) => p,
                None => return false,
            },
            _ => d,
        };
        ast.closure_thunk(f).is_some()
            || ast.decls.get(f).ctx().is_some_and(|c| {
                ast.closure_invoker(c).is_some_and(|i| ast.first(i) == ast.first(f))
                    || (ast.closure_lambda(c).is_some()
                        && ast.decls.get(f).kind() == DeclKind::CXXConversion)
            })
    }

    /// Whether `d` is a closure's capture member (an unnamed field).
    pub(super) fn capture_field(&self, d: DeclId) -> bool {
        let ast = &self.s.ast;
        ast.decls
            .get(d)
            .ctx()
            .and_then(|c| ast.closure_lambda(c))
            .is_some_and(|l| ast.exprs.lambdas.get(l).captures.iter().any(|c| c.field == Some(d)))
    }

    /// A local class's or enumeration's name word: a local's (`pnames`,
    /// not gated); `None` for any other declaration.
    pub(super) fn local_tag_name(&mut self, d: DeclId) -> W<Option<u32>> {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        if !matches!(n.kind(), DeclKind::CXXRecord | DeclKind::Enum)
            || n.ctx().is_none_or(|c| ast.decls.function(c).is_none())
        {
            return Ok(None);
        }
        Ok(Some(match n.name() {
            x if x == kiln_ast::names::DeclName::EMPTY => NONE,
            x => {
                let s = self.ident(x).ok_or("a local class not named by an identifier")?;
                Self::name_ix(&mut self.pnames, &mut self.img.pnames, &s)
            }
        }))
    }

    /// The record of `d` when it is a lazy tag or in one, writing the
    /// tag first; `None` when `d` is in no tag, or the tag does not
    /// record it (an implicit instantiation of a member template).
    pub(super) fn lazy_ref(&mut self, d: DeclId) -> W<Option<u32>> {
        let Some(t) = self.tag_of(d) else { return Ok(None) };
        let t = self.s.ast.first(t);
        if !self.recs.contains_key(&t) {
            self.write_tag(t)?;
        }
        Ok(self.recs.get(&d).copied())
    }

    /// Writes tag `t` if type `d`'s declaration is one (`ty`'s record
    /// and enumeration types).
    pub(super) fn lazy_type(&mut self, d: DeclId) -> W<()> {
        if !self.recs.contains_key(&d) && self.is_tag(d) {
            self.write_tag(d)?;
        }
        Ok(())
    }

    /// Writes tag `t` and everything declared in it.
    fn write_tag(&mut self, t: DeclId) -> W<()> {
        let ast = &self.s.ast;
        if self.bs.texpr > 0 {
            return Err("a lambda expression or local class in a type");
        }
        if !self.bs.lam.writing.is_empty() {
            return Err("a closure or local class named while another is recorded");
        }
        if ast.first(t) != t || ast.definition(t) != Some(t) {
            return Err("a local class or enumeration declared before its definition");
        }
        if let Some(c) = ast.decls.get(t).ctx()
            && c != ast.tu
            && !self.recs.contains_key(&c)
        {
            return Err("a closure or local class named before its context is recorded");
        }
        let lambda = ast.closure_lambda(t);
        if let Some(l) = lambda {
            // The captured entities first: their records precede the
            // closure's.
            let caps: Vec<LambdaCapture> = ast.exprs.lambdas.get(l).captures.clone();
            for c in caps {
                if let (Some(v), false) = (c.var, c.init) {
                    self.dref(v, Dc::Value)?;
                }
            }
        }
        if let Some(&why) = self.bs.lam.failed.get(&t) {
            return Err(why);
        }
        let m = self.marks();
        let np = self.bs.param_pos.len();
        self.bs.lam.writing.push(t);
        let r = self.tag_records(t);
        self.bs.lam.writing.pop();
        let r = r.and_then(|()| match lambda {
            Some(l) => self.lambda_rec(t, l),
            None => Ok(()),
        });
        let r = r.and_then(|()| self.tag_patches(m.decls as u32));
        if let Err(why) = r {
            self.rollback(m);
            // The parameters the dropped functions numbered.
            self.bs.param_pos.truncate(np);
            self.bs.params.retain(|_, v| (*v as usize) < np);
            self.bs.lam.failed.insert(t, why);
        }
        r
    }

    /// Drops the lambda records whose closures a rollback dropped.
    pub(super) fn lam_rollback(&mut self) {
        let n = self.img.decls.len() as u32;
        self.img.lambdas.truncate(n);
        let len = self.img.lambdas.recs.len() as u32;
        self.bs.lam.index.retain(|_, v| *v < len);
    }

    /// The declarations in tag `t`, `t` first, in id order.
    fn tag_decls(&self, t: DeclId) -> (Vec<DeclId>, FxHashSet<DeclId>) {
        let ast = &self.s.ast;
        // A member template's parameters, declared in the scope the
        // lambda is in (a generic lambda's invented parameters).
        let mut heads = FxHashSet::default();
        let mut hi = t.index();
        let mut stack = vec![t];
        let mut steps = 0u32;
        while let Some(x) = stack.pop() {
            steps += 1;
            if steps > 1 << 20 {
                break;
            }
            let Some(def) = ast.definition(x) else { continue };
            for m in ast.members(def) {
                hi = hi.max(m.index());
                if let Some(info) = ast.template_info(m) {
                    let ps = ast.templates.param_list(info.params).params;
                    heads.extend(ast.lists.iter::<DeclId>(ps));
                }
                if let Some(p) = ast.template_info(m).and_then(|i| i.pattern) {
                    hi = hi.max(p.index());
                    stack.push(p);
                }
                if matches!(ast.decls.get(m).kind(), DeclKind::CXXRecord | DeclKind::Enum) {
                    stack.push(m);
                }
            }
        }
        let v = (t.index()..=hi)
            .map(DeclId::new)
            .filter(|&x| heads.contains(&x) || self.tag_of(x) == Some(t))
            .collect();
        (v, heads)
    }

    /// The records of tag `t` ([`Writer::write_tag`]).
    fn tag_records(&mut self, t: DeclId) -> W<()> {
        let mut st = Pass::default();
        let (ds, heads) = self.tag_decls(t);
        self.bs.lam.heads = heads;
        let r = ds.into_iter().try_for_each(|d| self.one(d, &mut st));
        self.bs.lam.heads.clear();
        r?;
        let Pass { mut open, implicit, .. } = st;
        while let Some(top) = open.pop() {
            self.close_class(top);
        }
        let ast = &self.s.ast;
        for (d, ctx, kind) in implicit {
            let o = ast.modules.owner(d);
            let i = self.img.decls.len() as u32;
            self.img.decls.push(DeclRec {
                flags: ast.decls.get(d).flags().bits() & DECL_FLAGS,
                owner: u8::from(o.purview) | (u8::from(o.exported) << 1),
                access: 0,
                name: NONE,
                ctx,
                lexical: NONE,
                loc: NONE,
                prev: NONE,
                payload: Payload::Implicit { kind },
            });
            self.recs.insert(d, i);
        }
        Ok(())
    }

    /// A declaration word for a capture's entity or a mangling context.
    fn lam_dref(&mut self, d: DeclId) -> W<u32> {
        if let Some(&r) = self.recs.get(&d) {
            return Ok(r);
        }
        if let Some(&p) = self.bs.params.get(&d) {
            return Ok(PARAM | p);
        }
        self.dref(d, Dc::Value)
    }

    /// The side record of lambda `l`, closure `t` (its records written).
    fn lambda_rec(&mut self, t: DeclId, l: LambdaId) -> W<()> {
        let data = self.s.ast.exprs.lambdas.get(l).clone();
        let closure = *self.recs.get(&t).ok_or("a malformed lambda")?;
        let call_op = match data.call_op {
            Some(op) => *self.recs.get(&op).ok_or("a lambda's call operator outside the image")?,
            None => NONE,
        };
        let mut caps = Vec::with_capacity(data.captures.len());
        for c in &data.captures {
            let var = match c.var {
                Some(v) => self.lam_dref(v)?,
                None => NONE,
            };
            let field = match c.field {
                Some(f) => *self.recs.get(&f).ok_or("a lambda's capture outside the image")?,
                None => NONE,
            };
            caps.push(CapRec {
                kind: cap_kind(c.kind),
                var,
                field,
                loc: self.loc(c.loc)?,
                ellipsis: self.loc(c.ellipsis)?,
                flags: (u8::from(c.implicit) * CAP_IMPLICIT) | (u8::from(c.init) * CAP_INIT),
            });
        }
        // A variable's initializer's lambda is written before the
        // variable's own record: its context is patched once every record
        // is (`lambda_contexts`).
        let k = self.img.lambdas.recs.len() as u32;
        let context = match data.mangling.context {
            Some(c) if self.recs.contains_key(&c) || self.bs.params.contains_key(&c) => {
                self.lam_dref(c)?
            }
            Some(c) => {
                self.bs.lam.pending.push((k, c));
                NONE
            }
            None => NONE,
        };
        let pack_decls: Vec<DeclId> = self.s.lambda_packs.get(&l).cloned().unwrap_or_default();
        let mut packs = Vec::with_capacity(pack_decls.len());
        for d in pack_decls {
            packs.push(self.dref(d, Dc::Pack)?);
        }
        let flags = (u8::from(data.explicit_params) * L_PARAMS)
            | (u8::from(data.explicit_result) * L_RESULT)
            | (u8::from(data.mutable) * L_MUTABLE)
            | (u8::from(data.inits.is_some()) * L_INITS);
        let rec = LambdaRec {
            closure,
            call_op,
            default: data.default as u8,
            default_loc: self.loc(data.default_loc)?,
            lbracket: self.loc(data.lbracket)?,
            rbracket: self.loc(data.rbracket)?,
            flags,
            number: data.mangling.number,
            context,
            caps,
            packs,
            inits: Vec::new(),
        };
        self.img.lambdas.recs.push(rec);
        self.bs.lam.index.insert(l, k);
        Ok(())
    }

    /// Resolves what the records from `start` on (a tag's) name in other
    /// records, which `collect` resolves for the others once all are
    /// written: overridden functions, key functions, ctor-initializer
    /// targets.
    fn tag_patches(&mut self, start: u32) -> W<()> {
        let ovs: Vec<(u32, Vec<DeclId>)> =
            self.overrides.extract_if(.., |x| x.0 >= start).collect();
        for (i, list) in ovs {
            let mut out = Vec::with_capacity(list.len());
            for o in list {
                out.push(*self.recs.get(&o).ok_or("an override of a function outside the image")?);
            }
            if let Payload::Function { overrides, .. } = &mut self.img.decls[i as usize].payload {
                *overrides = out;
            }
        }
        let keys: Vec<(u32, DeclId)> = self.keys.extract_if(.., |x| x.0 >= start).collect();
        for (i, k) in keys {
            let r = *self.recs.get(&k).ok_or("a key function outside the image")?;
            if let Payload::Record { def: Some(df), .. } = &mut self.img.decls[i as usize].payload {
                df.key = r;
            }
        }
        let inits: Vec<(u32, u32, DeclId)> =
            self.bs.init_fields.extract_if(.., |x| x.0 >= start).collect();
        for (i, k, f) in inits {
            let r = *self.recs.get(&f).ok_or("a ctor-initializer of a field outside the image")?;
            if let Payload::Function { inits, .. } = &mut self.img.decls[i as usize].payload
                && let Some(x) = inits.get_mut(k as usize)
            {
                x.target = r;
            }
        }
        let end = self.img.decls.len() as u32;
        self.bs.lam.ranges.push((start, end));
        Ok(())
    }

    /// Patches the mangling contexts recorded after their lambdas.
    pub(super) fn lambda_contexts(&mut self) -> W<()> {
        // The tags' declarator locations.
        let ranges = std::mem::take(&mut self.bs.lam.ranges);
        let n = self.img.decls.len() as u32;
        let mut recs: Vec<(u32, DeclId)> = self
            .recs
            .iter()
            .filter(|&(_, &i)| i < n && ranges.iter().any(|&(a, b)| (a..b).contains(&i)))
            .map(|(&d, &i)| (i, d))
            .collect();
        recs.sort_unstable();
        self.more_decl_locs(&recs)?;
        for (k, c) in std::mem::take(&mut self.bs.lam.pending) {
            let Some(r) = self.img.lambdas.recs.get(k as usize) else { continue };
            let closure = r.closure;
            match self.recs.get(&c) {
                Some(&w) => self.img.lambdas.recs[k as usize].context = w,
                None => {
                    self.miss(closure, "a lambda whose mangling context is outside the image")?;
                }
            }
        }
        Ok(())
    }

    /// The `IMPLICIT` flag a head keeps: an invented template parameter's
    /// (WP-1345), or an implicit member of a lazy tag being written.
    pub(super) fn invented_bits(&self, d: DeclId) -> u8 {
        let n = self.s.ast.decls.get(d);
        if n.kind() == DeclKind::TemplateTypeParm || self.in_lazy_write(d) && self.lazy_implicit(d)
        {
            n.flags().bits() & DeclFlags::IMPLICIT.bits()
        } else {
            0
        }
    }

    /// The [`F::LB`](super::body::F::LB) word of lambda `l`, whose
    /// capture initializers are written: its side record, the closure
    /// written first.
    pub(super) fn lambda_word(&mut self, l: LambdaId) -> W<u32> {
        let data = self.s.ast.exprs.lambdas.get(l);
        let closure = self.s.ast.first(data.closure);
        let inits = data.inits;
        if self.bs.texpr > 0 {
            return Err("a lambda expression in a type");
        }
        if !self.recs.contains_key(&closure) {
            self.write_tag(closure)?;
        }
        let k = *self.bs.lam.index.get(&l).ok_or("a malformed lambda")?;
        if let Some(list) = inits {
            let es: Vec<ExprId> = self.s.ast.lists.iter::<ExprId>(list).collect();
            let mut v = Vec::with_capacity(es.len());
            for e in es {
                v.push(self.expr_word(e)?);
            }
            self.img.lambdas.recs[k as usize].inits = v;
        }
        Ok(k)
    }

    /// The capture initializers of lambda `l` (the lambda-expression's
    /// operands).
    pub(super) fn lambda_kids(&self, l: LambdaId) -> Vec<ExprId> {
        self.s
            .ast
            .exprs
            .lambdas
            .get(l)
            .inits
            .map_or_else(Vec::new, |list| self.s.ast.lists.iter::<ExprId>(list).collect())
    }
}

// ---------------------------------------------------------------------------
// The format
// ---------------------------------------------------------------------------

pub(super) fn encode_lambdas(w: &mut Wr, s: &Lambdas) {
    w.len(s.recs.len());
    for r in &s.recs {
        for x in [r.closure, r.call_op, r.default_loc, r.lbracket, r.rbracket, r.number, r.context]
        {
            w.u32(x);
        }
        w.u8(r.default);
        w.u8(r.flags);
        w.len(r.caps.len());
        for c in &r.caps {
            w.u8(c.kind);
            w.u8(c.flags);
            for x in [c.var, c.field, c.loc, c.ellipsis] {
                w.u32(x);
            }
        }
        w.len(r.packs.len());
        for &p in &r.packs {
            w.u32(p);
        }
        w.len(r.inits.len());
        for &e in &r.inits {
            w.u32(e);
        }
    }
}

pub(super) fn decode_lambdas(r: &mut Rd) -> Res<Lambdas> {
    let n = r.count(34)?;
    let mut recs = Vec::with_capacity(n);
    for _ in 0..n {
        let mut w = [0u32; 7];
        for x in &mut w {
            *x = r.u32()?;
        }
        let [closure, call_op, default_loc, lbracket, rbracket, number, context] = w;
        let default = r.u8()?;
        let flags = r.u8()?;
        let nc = r.count(18)?;
        let mut caps = Vec::with_capacity(nc);
        for _ in 0..nc {
            let kind = r.u8()?;
            let cflags = r.u8()?;
            let (var, field, loc, ellipsis) = (r.u32()?, r.u32()?, r.u32()?, r.u32()?);
            caps.push(CapRec { kind, var, field, loc, ellipsis, flags: cflags });
        }
        let np = r.count(4)?;
        let mut packs = Vec::with_capacity(np);
        for _ in 0..np {
            packs.push(r.u32()?);
        }
        let ni = r.count(4)?;
        let mut inits = Vec::with_capacity(ni);
        for _ in 0..ni {
            inits.push(r.u32()?);
        }
        recs.push(LambdaRec {
            closure,
            call_op,
            default,
            default_loc,
            lbracket,
            rbracket,
            flags,
            number,
            context,
            caps,
            packs,
            inits,
        });
    }
    Ok(Lambdas { recs })
}

/// Every main-file offset the section holds.
pub(super) fn lambda_locs(img: &Image, offs: &mut Vec<u32>) {
    for r in &img.lambdas.recs {
        offs.extend([r.default_loc, r.lbracket, r.rbracket]);
        offs.extend(r.caps.iter().flat_map(|c| [c.loc, c.ellipsis]));
    }
}

/// Whether record `i` is a closure type's definition.
pub(super) fn closure_def(img: &Image, i: u32) -> bool {
    img.decls.get(i as usize).is_some_and(|d| {
        matches!(&d.payload, Payload::Record { def: Some(df), .. }
            if df.bits & DefinitionBits::LAMBDA.bits() != 0)
    })
}

/// Whether record `x` is declared in closure record `c`.
fn member_of(img: &Image, x: u32, c: u32) -> bool {
    img.decls.get(x as usize).is_some_and(|d| d.ctx == c)
}

/// Whether record `i` may carry `IMPLICIT`: a closure type's definition,
/// or a function or template declared in one (its conversion function
/// and static invoker; sema's other implicit members are `Implicit`
/// records).
pub(super) fn implicit_ok(img: &Image, i: u32) -> bool {
    closure_def(img, i)
        || img.decls.get(i as usize).is_some_and(|d| {
            matches!(d.payload, Payload::Function { .. } | Payload::Template { .. })
                && closure_def(img, d.ctx)
        })
}

/// The section's checks: each record names a distinct closure
/// definition, its members and entities; every closure definition has a
/// record.
pub(super) fn validate_lambdas(img: &Image, nparams: usize) -> Res<()> {
    let ne = img.bodies.exprs.len();
    let nd = img.decls.len() as u32;
    let mut seen = FxHashSet::default();
    let word_ok = |w: u32| {
        w == NONE
            || (w & PARAM != 0 && ((w & !PARAM) as usize) < nparams)
            || (w & PARAM == 0 && w < nd)
    };
    for r in &img.lambdas.recs {
        let call_ok = r.call_op == NONE
            || (member_of(img, r.call_op, r.closure)
                && matches!(img.decls[r.call_op as usize].payload, Payload::Function { .. }));
        let caps_ok = r.caps.iter().all(|c| {
            c.kind <= 3
                && c.flags <= CAP_IMPLICIT | CAP_INIT
                && word_ok(c.var)
                && (c.var == NONE) == (c.kind <= 1)
                && (c.field == NONE
                    || (member_of(img, c.field, r.closure)
                        && matches!(img.decls[c.field as usize].payload, Payload::Field { .. })))
        });
        if !closure_def(img, r.closure)
            || !seen.insert(r.closure)
            || !call_ok
            || !caps_ok
            || r.default > 2
            || r.flags > L_PARAMS | L_RESULT | L_MUTABLE | L_INITS
            || !word_ok(r.context)
            || (r.flags & L_INITS == 0 && !r.inits.is_empty())
            || r.inits.iter().any(|&e| e as usize >= ne)
            || r.packs.iter().any(|&p| p == NONE || !super::body::dc_ok(img, nparams, p, Dc::Pack))
        {
            return bad("bad lambda record");
        }
    }
    let defs = (0..nd).filter(|&i| closure_def(img, i)).count();
    if defs != seen.len() {
        return bad("a closure type without its lambda");
    }
    Ok(())
}

/// The [`F::LB`](super::body::F::LB) token check: a lambda record whose
/// capture initializers precede the expression.
pub(super) fn lb_ok(img: &Image, w: u32, e_lim: usize) -> bool {
    img.lambdas
        .recs
        .get(w as usize)
        .is_some_and(|r| r.flags & L_INITS == 0 || r.inits.iter().all(|&e| (e as usize) < e_lim))
}

// ---------------------------------------------------------------------------
// The importer
// ---------------------------------------------------------------------------

impl Sema<'_, '_> {
    /// A closure type's record: the class and its (empty) lambda; the
    /// rest of the lambda is filled by [`Sema::image_lambdas`].
    pub(super) fn image_closure(&mut self, h: DeclHead, dependent: bool) -> DeclId {
        let c = self.builder().closure_record(h, !dependent);
        let id = self.ast.exprs.lambdas.push(LambdaData::new(c, h.loc));
        self.ast.decls.set_closure(c, id);
        c
    }

    /// Fills each lambda's side record once every record is built.
    pub(super) fn image_lambdas(
        &mut self,
        img: &Image,
        ids: &[DeclId],
        params: &[DeclId],
        main: &super::Locs,
    ) {
        let decl = |w: u32| -> Option<DeclId> {
            if w == NONE {
                None
            } else if w & PARAM != 0 {
                params.get((w & !PARAM) as usize).copied()
            } else {
                ids.get(w as usize).copied()
            }
        };
        for r in &img.lambdas.recs {
            let Some(l) = self.ast.decls.record(ids[r.closure as usize]).and_then(|x| x.lambda)
            else {
                continue;
            };
            let captures: Vec<LambdaCapture> = r
                .caps
                .iter()
                .map(|c| LambdaCapture {
                    kind: kind_of_cap(c.kind),
                    var: decl(c.var),
                    field: decl(c.field),
                    loc: main.get(c.loc),
                    ellipsis: main.get(c.ellipsis),
                    implicit: c.flags & CAP_IMPLICIT != 0,
                    init: c.flags & CAP_INIT != 0,
                })
                .collect();
            let x = self.ast.exprs.lambdas.get_mut(l);
            x.call_op = decl(r.call_op);
            x.default = match r.default {
                1 => CaptureDefault::ByCopy,
                2 => CaptureDefault::ByRef,
                _ => CaptureDefault::None,
            };
            x.default_loc = main.get(r.default_loc);
            x.captures = captures;
            x.lbracket = main.get(r.lbracket);
            x.rbracket = main.get(r.rbracket);
            x.explicit_params = r.flags & L_PARAMS != 0;
            x.explicit_result = r.flags & L_RESULT != 0;
            x.mutable = r.flags & L_MUTABLE != 0;
            x.mangling.number = r.number;
            x.mangling.context = decl(r.context);
            let packs: Vec<DeclId> = r.packs.iter().filter_map(|&w| decl(w)).collect();
            if !packs.is_empty() {
                self.lambda_packs.insert(l, packs);
            }
        }
    }

    /// An [`F::LB`](super::body::F::LB) slot: the lambda of record `k`,
    /// its capture initializers set.
    pub(super) fn image_lambda_inits(
        &mut self,
        img: &Image,
        ids: &[DeclId],
        built: &Built,
        k: u32,
    ) -> u32 {
        let Some(r) = img.lambdas.recs.get(k as usize) else { return 0 };
        let Some(l) = self.ast.decls.record(ids[r.closure as usize]).and_then(|x| x.lambda) else {
            return 0;
        };
        if r.flags & L_INITS != 0 {
            let es: Vec<ExprId> = r.inits.iter().map(|&e| built.expr(e)).collect();
            let list: ListId = self.ast.lists.push(&es);
            self.ast.exprs.lambdas.get_mut(l).inits = Some(list);
        }
        l.raw()
    }

    /// Each lambda's compound-statement: its call operator's body.
    pub(super) fn image_lambda_bodies(&mut self, img: &Image, ids: &[DeclId]) {
        for r in &img.lambdas.recs {
            let c = ids[r.closure as usize];
            let Some(l) = self.ast.decls.record(c).and_then(|x| x.lambda) else { continue };
            let body = self
                .ast
                .exprs
                .lambdas
                .get(l)
                .call_op
                .and_then(|f| self.ast.decls.function(f))
                .and_then(|f| f.body);
            self.ast.exprs.lambdas.get_mut(l).body = body;
        }
    }
}
