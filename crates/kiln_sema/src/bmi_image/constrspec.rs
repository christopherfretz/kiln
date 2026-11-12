//! Specializations of constrained templates in the declaration image
//! (spec/modules.md §11.11, "Forms carried by WP-1344").
//!
//! The importer resolves a specialization reference while it builds the
//! records, before the constraints are patched in (they are expressions
//! of the bodies, [`super::constraints`]). Of a template's constraints,
//! only some are read then:
//!
//! - the template's own (its head's requires-clause and its parameters'
//!   type-constraints) are checked by `get_class_spec`; a specialization
//!   the unit made satisfied them ([temp.names]/8), so checking nothing
//!   finds the same specialization (what replay's check cached is not
//!   part of the program);
//! - a constrained partial specialization's are read by matching: the
//!   image records the unit's selection for a specialization the unit
//!   completed (WP-1358, [`super::psel`]), and the importer deduces the
//!   arguments of that partial specialization only;
//! - a member's (a trailing requires-clause, a member template's head) are
//!   copied into the class's instantiated members when the importer
//!   completes the class; a specialization the unit completed would be
//!   completed without them, so it stays refused when the pattern it was
//!   instantiated from has one. One the unit left incomplete is completed
//!   (if ever) after the constraints.

use kiln_ast::DeclId;
use kiln_ast::decl::DeclKind;
use kiln_ast::templates::SpecInfo;

use kiln_ast::decl::DefinitionBits;

use super::{Image, NONE, Payload, Writer};
use crate::Sema;

/// The census reason of a completed specialization of a class template
/// with a constrained member.
const MEMBER: &str =
    "a specialization of a constrained template: completed, with a constrained member";

impl Writer<'_, '_, '_> {
    /// Why specialization of template `t` (completed by the unit when
    /// `complete`, instantiated from pattern `from`) is refused for `t`'s
    /// constraints, or `None`.
    pub(super) fn constrained_spec_why(
        &self,
        t: DeclId,
        complete: bool,
        from: DeclId,
    ) -> Option<&'static str> {
        if !complete {
            return None;
        }
        let ast = &self.s.ast;
        let from = ast.definition(from).unwrap_or(from);
        // Member templates differing only in their constraints matter
        // whatever the class template's own: the importer's completion,
        // without them, would declare the second as the first
        // ([temp.over.link]/7; a specialization's own member template is
        // refused, [`Self::constrained_member_template`]).
        let why = if self.constrained_template(t) {
            self.constrained_members(from)
        } else {
            self.constrained_template_overloads(from)
        };
        if why { Some(MEMBER) } else { None }
    }

    /// The selection to record for specialization `si` (complete when
    /// `complete`, [`super::psel`]): its partial specialization's record,
    /// or [`NONE`] for the primary template, when its template has a
    /// constrained partial specialization; `None` otherwise. `spec_rec`
    /// refused it before when its partial specialization is not carried.
    pub(super) fn partial_sel_of(&self, si: &SpecInfo, complete: bool) -> Option<u32> {
        if !complete || !self.constrained_partials(si.template) {
            return None;
        }
        if si.from_args.is_none() {
            return Some(NONE);
        }
        self.recs.get(&self.s.ast.first(si.from)).copied()
    }

    /// Whether class template `t` has a partial specialization with
    /// constraints ([temp.spec.partial.match]/2).
    pub(super) fn constrained_partials(&self, t: DeclId) -> bool {
        let ast = &self.s.ast;
        let tt = &ast.templates;
        tt.partials_of(ast.first(t)).iter().any(|&k| {
            tt.partial(k).is_some_and(|p| {
                let pl = tt.param_list(p.params);
                pl.requires.is_some()
                    || ast
                        .lists
                        .iter::<DeclId>(pl.params)
                        .any(|q| ast.template_parm(q).is_some_and(|x| x.constraint.is_some()))
            })
        })
    }

    /// Whether class pattern `p` has a constrained member the importer
    /// cannot give its constraints after they are patched in
    /// ([`Sema::image_member_constraints`]): a member template (at any
    /// depth) with a requires-clause or a constrained parameter; a member
    /// of a nested class, or of a member template, with a trailing
    /// requires-clause; a member function of `p` with one that is a
    /// constructor, destructor or special member, or shares its name with
    /// another member.
    fn constrained_members(&self, p: DeclId) -> bool {
        let ast = &self.s.ast;
        let tt = &ast.templates;
        let inside_p = |f: DeclId| ast.decls.get(f).ctx() == Some(p);
        // In a class nested (at any depth) in `p`.
        let nested = |f: DeclId| {
            let mut c = ast.decls.get(f).ctx();
            let mut depth = 0;
            for _ in 0..64 {
                match c {
                    Some(x) if x == p => return depth > 0,
                    Some(x) if ast.decls.record(x).is_some() => {
                        depth += 1;
                        c = ast.decls.get(x).ctx();
                    }
                    _ => return false,
                }
            }
            false
        };
        // A direct member whose constraints the importer's completion
        // needs (WP-1358): another member of its name, declared without
        // them, would redeclare it ([basic.scope.scope]/4, overloads
        // differing only in constraints), and a special member's decide
        // which is eligible ([special]/6).
        let clash = |f: DeclId| {
            let entity = ast.described_template(f).unwrap_or(f);
            let name = ast.decls.get(entity).name();
            entity != f
                || self.s.special_kind_of(f).is_some()
                || matches!(
                    ast.decls.get(f).kind(),
                    DeclKind::CXXConstructor | DeclKind::CXXDestructor
                )
                || ast.members(p).any(|m| {
                    let e = ast.described_template(m).unwrap_or(m);
                    e != entity && ast.decls.get(e).name() == name
                })
        };
        tt.trailing_requires.iter().any(|(f, _)| nested(f) || inside_p(f) && clash(f))
            || self.constrained_member_templates(p)
    }

    /// Whether class `p` or a class nested in it has a member template
    /// with constraints.
    fn constrained_member_templates(&self, p: DeclId) -> bool {
        self.member_templates_any(p, |w, _, m| w.head_constrained(m))
    }

    /// Whether class `p` or a class nested in it has a constrained member
    /// function template with the same name and function type as another
    /// member function template of its class (overloads differing only
    /// in their constraints, [temp.over.link]/7).
    fn constrained_template_overloads(&self, p: DeclId) -> bool {
        let ast = &self.s.ast;
        let sig = |m: DeclId| {
            let i = ast.template_info(m)?;
            let f = i.pattern?;
            let n = ast.lists.iter::<DeclId>(ast.templates.param_list(i.params).params).count();
            Some((ast.decls.get(m).name(), ast.types.canonical(ast.decls.get(f).ty()?), n))
        };
        self.member_templates_any(p, |w, c, m| {
            ast.decls.get(m).kind() == DeclKind::FunctionTemplate
                && w.head_constrained(m)
                && sig(m).is_some_and(|x| {
                    ast.members(c).any(|o| {
                        o != m
                            && ast.decls.get(o).kind() == DeclKind::FunctionTemplate
                            && sig(o) == Some(x)
                    })
                })
        })
    }

    /// Whether template `m`'s head has constraints (a requires-clause or a
    /// type-constraint).
    pub(super) fn head_constrained(&self, m: DeclId) -> bool {
        let ast = &self.s.ast;
        ast.template_info(m).is_some_and(|i| {
            let l = ast.templates.param_list(i.params);
            l.requires.is_some()
                || ast
                    .lists
                    .iter::<DeclId>(l.params)
                    .any(|q| ast.template_parm(q).is_some_and(|x| x.constraint.is_some()))
        })
    }

    /// Whether `f(self, class, member template)` holds for a member
    /// template of class `p` or of a class nested in it (true past 4096
    /// classes).
    fn member_templates_any(&self, p: DeclId, f: impl Fn(&Self, DeclId, DeclId) -> bool) -> bool {
        let ast = &self.s.ast;
        let mut stack = vec![p];
        let mut n = 0;
        while let Some(c) = stack.pop() {
            n += 1;
            if n > 4096 {
                return true;
            }
            for m in ast.members(c) {
                let k = ast.decls.get(m).kind();
                if matches!(
                    k,
                    DeclKind::FunctionTemplate
                        | DeclKind::ClassTemplate
                        | DeclKind::VarTemplate
                        | DeclKind::TypeAliasTemplate
                ) && let Some(i) = ast.template_info(m)
                {
                    if f(self, c, m) {
                        return true;
                    }
                    if let Some(pp) = i.pattern
                        && ast.decls.record(pp).is_some()
                    {
                        stack.push(pp);
                    }
                }
                if k == DeclKind::CXXRecord {
                    stack.push(m);
                }
            }
        }
        false
    }
}

/// The importer's side.
impl Sema<'_, '_> {
    /// Gives the members of the class specializations the image completed
    /// while it read the records, before the constraints were patched in,
    /// what their completion copies from a constrained pattern member
    /// (WP-1358, `Substituter::function_decl`): a member function's
    /// trailing requires-clause, kept with the specialization's levels
    /// (`member_constraints`), as a replay's completion did.
    pub(super) fn image_member_constraints(&mut self, img: &Image, ids: &[DeclId]) {
        for (i, r) in img.decls.iter().enumerate() {
            if !matches!(r.payload, Payload::Spec { complete: true, .. }) {
                continue;
            }
            let Some(&spec) = ids.get(i) else { continue };
            let spec = self.ast.first(spec);
            if !self
                .ast
                .decls
                .record(spec)
                .is_some_and(|x| x.defn.contains(DefinitionBits::COMPLETE))
            {
                continue;
            }
            let Some((_, args)) = self.spec_args(spec) else { continue };
            let members: Vec<DeclId> = self.ast.members(spec).collect();
            for m in members {
                if self.inst.concepts.member_constraints.contains_key(&m) {
                    continue;
                }
                if let Some(t) = self.ast.templates.instantiated_from_map(m) {
                    self.image_member_template_head(m, t, &args);
                    continue;
                }
                if self.ast.decls.function(m).is_some()
                    && let Some(d) = self.ast.instantiated_from(m)
                    && let Some(&e) = self.ast.templates.trailing_requires.get(d)
                {
                    let _ = self.ast.templates.trailing_requires.insert(m, e);
                    self.inst.concepts.member_constraints.insert(m, (d, args.clone()));
                }
            }
        }
    }

    /// Gives member template `m` of a completed class specialization,
    /// instantiated from the pattern's member template `t`, what the
    /// completion copies from `t`'s head (`Substituter::param_list`,
    /// `member_template`): its parameters' type-constraints and its
    /// requires-clause, unsubstituted, and `t` with the specialization's
    /// levels `args` in `member_constraints` (WP-1358).
    fn image_member_template_head(
        &mut self,
        m: DeclId,
        t: DeclId,
        args: &crate::templ::args::MultiLevelArgs,
    ) {
        let (Some(mi), Some(ti)) =
            (self.ast.template_info(m).copied(), self.ast.template_info(t).copied())
        else {
            return;
        };
        let ml = *self.ast.templates.param_list(mi.params);
        let tl = *self.ast.templates.param_list(ti.params);
        let mp: Vec<DeclId> = self.ast.lists.iter::<DeclId>(ml.params).collect();
        let tp: Vec<DeclId> = self.ast.lists.iter::<DeclId>(tl.params).collect();
        if mp.len() != tp.len() {
            return;
        }
        for (&np, &op) in mp.iter().zip(&tp) {
            let c = self.ast.template_parm(op).and_then(|x| x.constraint);
            if let Some(c) = c
                && self.ast.template_parm(np).is_some_and(|x| x.constraint.is_none())
            {
                self.builder().set_parm_constraint(np, c);
            }
        }
        if ml.requires.is_none()
            && let Some(e) = tl.requires
        {
            self.builder().set_param_list_requires(mi.params, e);
        }
        if !self.associated_constraints(t).is_empty() {
            self.inst.concepts.member_constraints.insert(m, (t, args.clone()));
        }
    }
}
