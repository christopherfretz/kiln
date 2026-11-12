//! Constraints and concepts in the declaration image (spec/modules.md
//! §11.11, "Constraints", WP-1284).
//!
//! A concept is a `Template` record of kind [`CONCEPT_KIND`] with no
//! pattern. Every constraint is an expression of the bodies section,
//! listed in [`Constraints`] with the record it belongs to and what it
//! is: a template parameter's immediately-declared constraint, a template
//! head's requires-clause, a function's trailing requires-clause (a
//! function template's pattern or a member of a carried class pattern),
//! or a concept's constraint-expression. The importer builds the records
//! first and patches the constraints in once the bodies are built, as the
//! parser attaches them; nothing reads a template's associated
//! constraints before then: a specialization reference to a constrained
//! template, which the importer would instantiate while it builds the
//! records, is refused ([`Writer::constrained_template`]).
//!
//! Satisfaction is never carried. A concept-id the unit checked (a
//! non-dependent one, with a satisfaction result) falls back; a dependent
//! one is carried unchecked and checked by the importer's instantiations
//! exactly as replay checks it ([temp.constr.constr]).

use kiln_ast::decl::DeclKind;
use kiln_ast::{DeclId, ExprId};

use super::body::{Built, Root};
use super::{Image, NONE, Payload, Rd, Res, W, Wr, Writer, bad};
use crate::Sema;

/// The `Template` record kind of a concept (0 to 3 are function, class,
/// alias and variable templates).
pub(super) const CONCEPT_KIND: u8 = 4;

const MALFORMED_PARTIAL: &str = "a malformed partial specialization";

/// A template parameter's immediately-declared constraint
/// ([temp.param]/4).
const K_PARM: u8 = 0;
/// A template head's requires-clause ([temp.pre]/9).
const K_HEAD: u8 = 1;
/// A function's trailing requires-clause ([dcl.decl.general]/4).
const K_TRAILING: u8 = 2;
/// A concept's constraint-expression ([temp.concept]/1).
const K_CONCEPT: u8 = 3;
/// A function's `explicit(expr)` ([dcl.fct.spec]/3; WP-1345, kept with
/// the constraints: an expression of the bodies section patched onto a
/// function record once the bodies are built).
const K_EXPLICIT: u8 = 4;

/// One constraint: its kind, its declaration record and its expression
/// record.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct ConsRec {
    kind: u8,
    decl: u32,
    pub(super) expr: u32,
}

/// The image's constraint section.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct Constraints {
    pub(super) recs: Vec<ConsRec>,
}

/// What the writer found for one declaration ([`Writer::constraint_roots`]),
/// committed with its record.
pub(super) type Pending = Vec<(u8, ExprId)>;

impl Writer<'_, '_, '_> {
    /// The constraints declaration `d` carries, or why it falls back.
    /// Pure: [`Writer::constraint_commit`] adds them once `d`'s record is
    /// in.
    pub(super) fn constraint_roots(&self, d: DeclId) -> W<Pending> {
        let s = self.s;
        let ast = &s.ast;
        let t = &ast.templates;
        let n = ast.decls.get(d);
        let mut v = Pending::new();
        match n.kind() {
            DeclKind::TemplateTypeParm | DeclKind::NonTypeTemplateParm => {
                if t.ttp_owner.contains(d) {
                    return Err("a type-constraint naming a concept template parameter");
                }
                if let Some(c) = ast.template_parm(d).and_then(|p| p.constraint) {
                    v.push((K_PARM, c));
                }
            }
            DeclKind::FunctionTemplate
            | DeclKind::ClassTemplate
            | DeclKind::TypeAliasTemplate
            | DeclKind::VarTemplate => {
                let info = ast.template_info(d).ok_or("a malformed template")?;
                if let Some(r) = t.param_list(info.params).requires {
                    v.push((K_HEAD, r));
                }
            }
            // A partial specialization's requires-clause (WP-1342,
            // [temp.spec.partial]/1): kind 1 on its record.
            DeclKind::CXXRecord | DeclKind::Var if ast.partial_spec_info(d).is_some() => {
                let (_, p) = ast.partial_spec_info(d).ok_or(MALFORMED_PARTIAL)?;
                if let Some(r) = t.param_list(p.params).requires {
                    v.push((K_HEAD, r));
                }
            }
            DeclKind::Concept => {
                let info = ast.template_info(d).ok_or("a malformed concept")?;
                // A concept merged with another unit's (`act_concept`) is
                // kept out of lookup, which the importer does not repeat.
                let ctx = n.ctx().ok_or("a malformed concept")?;
                let table = ast.primary_context(ast.redecl_context(ctx));
                if !ast.lookup.lookup_local(table, n.name()).iter().any(|x| x == d) {
                    return Err("a concept merged with another unit's");
                }
                v.push((K_CONCEPT, info.constraint.ok_or("a malformed concept")?));
            }
            _ => {}
        }
        if let Some(&r) = t.trailing_requires.get(d) {
            // Only a templated function has one ([dcl.decl.general]/5);
            // the unit diagnosed any other.
            if s.inst.concepts.untemplated_requires.contains(&d) || ast.decls.function(d).is_none()
            {
                return Err("a requires-clause on a non-templated function");
            }
            v.push((K_TRAILING, r));
        }
        if let Some(&e) = ast.decls.cold.explicit_expr.get(d) {
            v.push((K_EXPLICIT, e));
        }
        Ok(v)
    }

    /// Adds `p` for record `rec` (its expressions patched by
    /// [`Writer::walk_roots`]).
    pub(super) fn constraint_commit(&mut self, rec: u32, p: Pending) {
        for (kind, e) in p {
            let k = self.img.cons.recs.len() as u32;
            self.img.cons.recs.push(ConsRec { kind, decl: rec, expr: NONE });
            self.bs.roots.push(Root::Cons(rec, k, e));
        }
    }

    /// Whether template `t` has constraints, or (a class template) a
    /// member that has: the importer would check them while it builds
    /// the records, before they are patched in.
    pub(super) fn constrained_template(&self, t: DeclId) -> bool {
        let ast = &self.s.ast;
        let tt = &ast.templates;
        let t = ast.first(t);
        let Some(info) = ast.template_info(t) else { return false };
        let list = tt.param_list(info.params);
        let parm_constrained = ast.lists.iter::<DeclId>(list.params).any(|p| {
            ast.template_parm(p).is_some_and(|x| x.constraint.is_some()) || tt.ttp_owner.contains(p)
        });
        if list.requires.is_some() || parm_constrained {
            return true;
        }
        // WP-1342: a partial specialization with constraints, which the
        // importer's matching would see without them.
        let partial_constrained = tt.partials_of(t).iter().any(|&k| {
            tt.partial(k).is_some_and(|p| {
                let pl = tt.param_list(p.params);
                pl.requires.is_some()
                    || ast
                        .lists
                        .iter::<DeclId>(pl.params)
                        .any(|q| ast.template_parm(q).is_some_and(|x| x.constraint.is_some()))
            })
        });
        if partial_constrained {
            return true;
        }
        let Some(pattern) = info.pattern else { return true };
        if tt.trailing_requires.contains(pattern) {
            return true;
        }
        // A member (at any depth) of a class pattern with a trailing
        // requires-clause.
        ast.decls.get(t).kind() == DeclKind::ClassTemplate
            && tt.trailing_requires.iter().any(|(f, _)| {
                let mut c = ast.decls.get(f).ctx();
                for _ in 0..64 {
                    match c {
                        Some(x) if x == pattern => return true,
                        Some(x) if ast.decls.record(x).is_some() => c = ast.decls.get(x).ctx(),
                        _ => return false,
                    }
                }
                false
            })
    }
}

pub(super) fn encode_constraints(w: &mut Wr, c: &Constraints) {
    w.len(c.recs.len());
    for r in &c.recs {
        w.u8(r.kind);
        w.u32(r.decl);
        w.u32(r.expr);
    }
}

pub(super) fn decode_constraints(r: &mut Rd<'_>) -> Res<Constraints> {
    let n = r.count(9)?;
    let mut c = Constraints { recs: Vec::with_capacity(n) };
    for _ in 0..n {
        let kind = r.u8()?;
        let decl = r.u32()?;
        let expr = r.u32()?;
        c.recs.push(ConsRec { kind, decl, expr });
    }
    Ok(c)
}

/// Checks the constraint section: each constraint belongs to a record
/// of its kind, at most once per kind; every concept has its
/// constraint-expression.
pub(super) fn validate_constraints(img: &Image) -> Res<()> {
    let nexpr = img.bodies.exprs.len();
    let mut seen = kiln_source::fx::FxHashSet::default();
    for r in &img.cons.recs {
        let Some(d) = img.decls.get(r.decl as usize) else {
            return bad("bad constraint record");
        };
        let ok = match (r.kind, &d.payload) {
            (K_PARM, Payload::Ttp { .. } | Payload::Nttp { .. })
            | (K_TRAILING | K_EXPLICIT, Payload::Function { .. }) => true,
            (K_HEAD, Payload::Template { kind, .. }) => *kind < CONCEPT_KIND,
            // A partial specialization's (WP-1342).
            (K_HEAD, Payload::Record { .. } | Payload::Var { .. }) => {
                super::classtempl::partial_of(img, r.decl).is_some()
            }
            (K_CONCEPT, Payload::Template { kind, .. }) => *kind == CONCEPT_KIND,
            _ => false,
        };
        if !ok || (r.expr as usize) >= nexpr || !seen.insert((r.kind, r.decl)) {
            return bad("bad constraint record");
        }
    }
    for (i, d) in img.decls.iter().enumerate() {
        if matches!(d.payload, Payload::Template { kind: CONCEPT_KIND, .. })
            && !seen.contains(&(K_CONCEPT, i as u32))
        {
            return bad("a concept without its constraint-expression");
        }
    }
    Ok(())
}

/// The importer's side.
impl Sema<'_, '_> {
    /// Attaches the imported constraints once the bodies are built
    /// (record `i` is declaration `ids[i]`).
    pub(super) fn image_constraints(&mut self, img: &Image, ids: &[DeclId], built: &Built) {
        for r in &img.cons.recs {
            let (Some(&d), e) = (ids.get(r.decl as usize), built.expr(r.expr)) else { continue };
            match r.kind {
                K_PARM => self.builder().set_parm_constraint(d, e),
                K_HEAD => {
                    if let Some(info) = self.ast.template_info(d) {
                        let l = info.params;
                        self.builder().set_param_list_requires(l, e);
                    } else if let Some((_, p)) = self.ast.partial_spec_info(d) {
                        // A partial specialization's (WP-1342).
                        let l = p.params;
                        self.builder().set_param_list_requires(l, e);
                    }
                }
                K_TRAILING => {
                    let _ = self.ast.templates.trailing_requires.insert(d, e);
                }
                K_EXPLICIT => {
                    let _ = self.ast.decls.cold.explicit_expr.insert(d, e);
                }
                _ => self.builder().set_concept_constraint(d, e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::template_sample;
    use super::super::{decode, encode};
    use super::*;

    /// `template_sample` (parameter record 10, pattern 12, template 13)
    /// with a type-constraint, a requires-clause and a trailing
    /// requires-clause, each a sample expression.
    fn cons_sample() -> Image {
        let mut img = template_sample();
        img.cons.recs = vec![
            ConsRec { kind: K_PARM, decl: 10, expr: 0 },
            ConsRec { kind: K_HEAD, decl: 13, expr: 1 },
            ConsRec { kind: K_TRAILING, decl: 12, expr: 2 },
        ];
        img
    }

    /// spec/modules.md §11.11 "Constraints" (WP-1284; [temp.constr.decl]):
    /// constraints round-trip; one on a record of the wrong kind, naming
    /// an expression the image does not have, or given twice is an error.
    #[test]
    fn constraint_records() {
        type Corrupt = (&'static str, fn(&mut Image));
        let img = cons_sample();
        assert_eq!(decode(&encode(&img)).as_ref(), Ok(&img));
        let corrupt: [Corrupt; 5] = [
            ("head on a function", |i| i.cons.recs[1].decl = 12),
            ("trailing on a template", |i| i.cons.recs[2].decl = 13),
            ("concept expression on a template", |i| i.cons.recs[1].kind = K_CONCEPT),
            ("expression out of range", |i| i.cons.recs[0].expr = 9999),
            ("twice", |i| i.cons.recs.push(i.cons.recs[0])),
        ];
        for (what, f) in corrupt {
            let mut img = cons_sample();
            f(&mut img);
            assert!(decode(&encode(&img)).is_err(), "{what}");
        }
    }
}
