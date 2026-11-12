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
//! - a constrained partial specialization's are read by matching, and a
//!   member's (a trailing requires-clause, a member template's head) are
//!   copied into the class's instantiated members, both when the
//!   importer completes the class; a specialization the unit completed
//!   would be completed without them, so it stays refused. One the unit
//!   left incomplete is completed (if ever) after the constraints.

use kiln_ast::DeclId;
use kiln_ast::decl::DeclKind;

use super::Writer;

/// The census reason of a completed specialization of a template with
/// a constrained partial specialization.
const PARTIAL: &str = "a specialization of a constrained template: \
completed, with a constrained partial specialization";
/// The census reason of a completed specialization of a class template
/// with a constrained member.
const MEMBER: &str =
    "a specialization of a constrained template: completed, with a constrained member";

impl Writer<'_, '_, '_> {
    /// Why specialization of template `t` (completed by the unit when
    /// `complete`) is refused for `t`'s constraints, or `None`.
    pub(super) fn constrained_spec_why(&self, t: DeclId, complete: bool) -> Option<&'static str> {
        if !self.constrained_template(t) || !complete {
            return None;
        }
        let ast = &self.s.ast;
        let tt = &ast.templates;
        let t = ast.first(t);
        let info = ast.template_info(t)?;
        let partial = tt.partials_of(t).iter().any(|&k| {
            tt.partial(k).is_some_and(|p| {
                let pl = tt.param_list(p.params);
                pl.requires.is_some()
                    || ast
                        .lists
                        .iter::<DeclId>(pl.params)
                        .any(|q| ast.template_parm(q).is_some_and(|x| x.constraint.is_some()))
            })
        });
        if partial {
            return Some(PARTIAL);
        }
        let Some(pattern) = info.pattern else { return Some(MEMBER) };
        if self.constrained_members(pattern) { Some(MEMBER) } else { None }
    }

    /// Whether class pattern `p` has a member (at any depth) with a
    /// trailing requires-clause, or a member template with a
    /// requires-clause or a constrained parameter.
    fn constrained_members(&self, p: DeclId) -> bool {
        let ast = &self.s.ast;
        let tt = &ast.templates;
        let inside = |f: DeclId| {
            let mut c = ast.decls.get(f).ctx();
            for _ in 0..64 {
                match c {
                    Some(x) if x == p => return true,
                    Some(x) if ast.decls.record(x).is_some() => c = ast.decls.get(x).ctx(),
                    _ => return false,
                }
            }
            false
        };
        if tt.trailing_requires.iter().any(|(f, _)| inside(f)) {
            return true;
        }
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
                    let l = tt.param_list(i.params);
                    if l.requires.is_some()
                        || ast
                            .lists
                            .iter::<DeclId>(l.params)
                            .any(|q| ast.template_parm(q).is_some_and(|x| x.constraint.is_some()))
                    {
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
