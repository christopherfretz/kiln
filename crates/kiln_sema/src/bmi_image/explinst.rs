//! Explicit instantiations of class templates in the declaration image
//! (spec/modules.md §11.11, "Explicit instantiations"; WP-1307).
//!
//! `extern template class X<int>;` and `template class X<int>;`
//! ([temp.explicit]) make no declaration of their own: sema finds or
//! makes the specialization, instantiates its definition (and, for a
//! definition, its members' definitions, [temp.explicit]/10) and marks
//! its kind. The writer skips the specialization and its instantiated
//! members, as it skips an implicit instantiation, and lists each
//! explicit instantiation in a section of its own: the template record,
//! the arguments as written, the directive's location, its lexical
//! scope and whether it is a definition. The reader runs each one,
//! in order, through `act_on_explicit_class_inst`, the parser's action,
//! once every record and body is built (an instantiation needs the
//! pattern's bodies) and before the specializations the unit used are
//! marked used, so an importer's uses see the same specialization kinds
//! as after a replay.
//!
//! Differences from a replay, by construction: the instantiations run
//! after the unit's last declaration instead of at the directive. In a
//! valid unit that changes nothing a program can observe: a partial or
//! explicit specialization declared after an instantiation that would
//! have used it is ill-formed ([temp.spec.partial.general]/1,
//! [temp.expl.spec]/7), and the end of the unit is a point of
//! instantiation too ([temp.point]/7).

use kiln_ast::decl::DeclKind;
use kiln_ast::modules::Owner;
use kiln_ast::templates::SpecKind;
use kiln_ast::types::{ArgView, TemplateArg};
use kiln_ast::{DeclId, QualType};

use super::{ArgRec, BUILTINS, Image, Locs, NONE, Payload, QRef, Rd, Res, W, Wr, Writer, bad};
use crate::Sema;

/// One explicit instantiation.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct ExplInst {
    /// The class template's record.
    template: u32,
    /// The arguments as written.
    args: Vec<ArgRec>,
    /// The template-name's location (the specialization's after it).
    loc: u32,
    /// The namespace record the directive is in, or [`NONE`] (the
    /// translation unit).
    lexical: u32,
    /// The owner bits of the declarations it makes.
    owner: u8,
    /// `template class` (not `extern template class`).
    definition: bool,
}

/// The section, in declaration order of the specializations.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct ExplInsts {
    insts: Vec<ExplInst>,
}

impl Writer<'_, '_, '_> {
    /// Whether `d` is a class template specialization the unit
    /// explicitly instantiated.
    pub(super) fn is_explicit_class_inst(&self, d: DeclId) -> bool {
        let ast = &self.s.ast;
        ast.decls.get(d).kind() == DeclKind::CXXRecord
            && ast.spec_info(d).is_some_and(|(_, si)| {
                matches!(
                    si.kind,
                    SpecKind::ExplicitInstantiationDeclaration
                        | SpecKind::ExplicitInstantiationDefinition
                )
            })
    }

    /// Lists explicit instantiation `d` (a class template specialization;
    /// the caller skips it and its members).
    pub(super) fn explicit_class_inst(&mut self, d: DeclId) -> W<()> {
        let s = self.s;
        let ast = &s.ast;
        let n = ast.decls.get(d);
        let (_, si) = ast.spec_info(d).ok_or("a malformed specialization")?;
        let si = *si;
        if n.has(kiln_ast::decl::DeclFlags::HAS_ATTRS) {
            return Err(super::ATTRIBUTED);
        }
        // Instantiated from the primary template's own pattern (not a
        // partial specialization or a member template).
        let from_template = ast.template_info(si.from).is_some()
            || ast.described_template(si.from).map(|t| ast.first(t)) == Some(si.template);
        if si.from_args.is_some() || !from_template {
            return Err("an explicit instantiation of a partial specialization");
        }
        if self.constrained_template(si.template) {
            return Err("a specialization of a constrained template");
        }
        let template = self.template_rec(si.template, 1)?;
        let written = ast.templates.args_as_written(d).ok_or("a malformed specialization")?;
        let written: Vec<TemplateArg> = ast.types.args(written).to_vec();
        let converted: Vec<TemplateArg> = ast.types.args(si.args).to_vec();
        let packs = converted.iter().any(|a| matches!(a.view(), ArgView::Pack(_)));
        let mut args = Vec::with_capacity(written.len());
        for (k, a) in written.into_iter().enumerate() {
            let c = if packs { None } else { converted.get(k).copied() };
            let a = self.written_arg(a, c).ok_or(EXPLICIT_INST_ARG)?;
            args.push(self.arg_rec(a).map_err(|_| EXPLICIT_INST_ARG)?);
        }
        let lex = ast.decls.lexical_ctx(d).or(n.ctx()).unwrap_or(ast.tu);
        let lexical = if lex == ast.tu {
            NONE
        } else {
            match self.recs.get(&lex) {
                Some(&r) if matches!(self.img.decls[r as usize].payload, Payload::Namespace) => r,
                // A cascade of the namespace's own reason.
                None if ast.decls.get(lex).kind() == DeclKind::Namespace => {
                    return Err("an explicit instantiation in a namespace outside the image");
                }
                _ => return Err("an explicit instantiation in an unsupported scope"),
            }
        };
        let o = ast.modules.owner(d);
        let owner = u8::from(o.purview) | (u8::from(o.exported) << 1);
        let loc = self.loc(n.loc())?;
        let definition = si.kind == SpecKind::ExplicitInstantiationDefinition;
        self.img.explinsts.insts.push(ExplInst { template, args, loc, lexical, owner, definition });
        Ok(())
    }
}

const EXPLICIT_INST_ARG: &str = "an explicit instantiation argument the image does not carry";

pub(super) fn encode_explicit_insts(w: &mut Wr, s: &ExplInsts) {
    w.len(s.insts.len());
    for x in &s.insts {
        w.u32(x.template);
        w.args(&x.args);
        w.u32(x.loc);
        w.u32(x.lexical);
        w.u8(x.owner | (u8::from(x.definition) << 2));
    }
}

pub(super) fn decode_explicit_insts(r: &mut Rd) -> Res<ExplInsts> {
    let n = r.count(17)?;
    let mut insts = Vec::with_capacity(n);
    for _ in 0..n {
        let template = r.u32()?;
        let args = r.args()?;
        let (loc, lexical, bits) = (r.u32()?, r.u32()?, r.u8()?);
        if bits > 7 {
            return bad("bad explicit instantiation");
        }
        insts.push(ExplInst {
            template,
            args,
            loc,
            lexical,
            owner: bits & 3,
            definition: bits & 4 != 0,
        });
    }
    Ok(ExplInsts { insts })
}

/// The section's locations, for the decoder's offset check.
pub(super) fn explicit_inst_locs(img: &Image, offs: &mut Vec<u32>) {
    offs.extend(img.explinsts.insts.iter().map(|x| x.loc));
}

/// Checks the section: each entry names a class template record, a
/// namespace record or the translation unit, and valid arguments
/// (`qref_ok`: a type reference of the image).
pub(super) fn validate_explicit_insts(img: &Image, qref_ok: &dyn Fn(QRef) -> bool) -> Res<()> {
    for x in &img.explinsts.insts {
        let tmpl = img.decls.get(x.template as usize).map(|t| &t.payload);
        let lex_ok = x.lexical == NONE
            || img
                .decls
                .get(x.lexical as usize)
                .is_some_and(|r| matches!(r.payload, Payload::Namespace));
        let args_ok = x.args.iter().all(|a| match *a {
            ArgRec::Type(q) => qref_ok(q),
            ArgRec::Value(q, _) => q.ty <= BUILTINS && q.quals == 0,
            // A template (WP-1342).
            ArgRec::Template(d, 0) => super::tttp::tname_rec(img, d),
            // The writer lists converted arguments, never expressions
            // or dependent names.
            ArgRec::Expr(..) | ArgRec::Template(..) | ArgRec::DepTemplate(..) => false,
        });
        if !matches!(tmpl, Some(Payload::Template { kind: 1, .. })) || !lex_ok || !args_ok {
            return bad("bad explicit instantiation");
        }
    }
    Ok(())
}

/// The importer's side.
impl Sema<'_, '_> {
    /// Runs the unit's explicit instantiations (after the bodies), as
    /// the parser's action does.
    pub(super) fn image_explicit_insts(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        main: &Locs,
    ) {
        let unit = self.module_current_unit();
        for x in &img.explinsts.insts {
            let tmpl = self.ast.first(ids[x.template as usize]);
            let mut args = Vec::with_capacity(x.args.len());
            for a in &x.args {
                let ty = self.image_ty(img, tys, ids, a.q());
                args.push(self.image_targ(ids, ty, a));
            }
            let loc = main.get(x.loc);
            let locs = vec![loc; args.len()];
            let lexical = if x.lexical == NONE { self.ast.tu } else { ids[x.lexical as usize] };
            let owner = Owner { unit, purview: x.owner & 1 != 0, exported: x.owner & 2 != 0 };
            let next = self.ast.decls.next_id();
            self.ast.modules.set_owner_from(next, owner);
            let saved = std::mem::replace(&mut self.ctx, lexical);
            let _ = self.act_on_explicit_class_inst(tmpl, &args, &locs, !x.definition, loc);
            self.ctx = saved;
        }
    }
}
