//! Specializations a declaration's record names (WP-1358, spec/modules.md
//! §11.11, "Forms carried by WP-1358").
//!
//! A declaration's record comes after the specialization references its
//! types name (`prescan`, S4b): a record must not add records before its
//! own. What `prescan` does not reach (the implicit default constructor
//! of `__priority_tag<1>`, named by `__priority_tag<1>()` in a trailing
//! return type's `decltype`) the record adds while it is written. The
//! writer then drops what the record wrote, writes the references it
//! named first, in the order it named them, and writes the record again:
//! the importer resolves the references before the declaration, as
//! replay made them before it finished declaring it.

use kiln_ast::DeclId;
use kiln_ast::decl::DeclKind;

use super::{Head, Marks, Pass, Payload, W, Writer};

/// What [`Writer::record_after_named`] restores: the census marks, and
/// the parameter and requires-expression numbering a record adds.
#[derive(Clone, Copy)]
pub(super) struct NamedMarks {
    marks: Marks,
    param_pos: usize,
    req_recs: usize,
    req_params: usize,
}

/// The census reason of a record that still names a new reference when
/// it is written again, or names something that is no reference.
pub(super) const USER: &str = "a specialization named while its user is recorded";

impl Writer<'_, '_, '_> {
    /// Closes, before declaration `d`'s specialization references are
    /// written, the open classes `d` (head `h`) is no member of, which
    /// `one` closes after its record anyway: a specialization of a
    /// partial specialization defined just before (`template <class T>
    /// struct traits<T *> {...}; using cp = traits<char *>::value;`) is
    /// written after the partial specialization's definition ends, as
    /// the importer must complete it.
    pub(super) fn close_before(&mut self, d: DeclId, h: &Head, st: &mut Pass) {
        let n = self.s.ast.decls.get(d);
        if matches!(n.kind(), DeclKind::EnumConstant | DeclKind::ClassTemplate)
            || n.misc().friend() != kiln_ast::decl::FriendKind::None
        {
            return;
        }
        while let Some(&top) = st.open.last() {
            let inside = st.open_last.get(&top).is_some_and(|&l| l > d);
            if h.ctx == top || h.lexical == top || inside {
                break;
            }
            self.close_class(top);
            let _ = st.open.pop();
        }
    }

    /// The marks to take just before a declaration's record.
    pub(super) fn named_marks(&self) -> NamedMarks {
        NamedMarks {
            marks: self.marks(),
            param_pos: self.bs.param_pos.len(),
            req_recs: self.img.reqs.recs.len(),
            req_params: self.img.reqs.nparams,
        }
    }

    /// Writes declaration `d`'s record again after the references its
    /// first attempt added past `before` (the marks taken just before
    /// it): the references first, then the record. Returns the record's
    /// number and payload.
    pub(super) fn record_after_named(
        &mut self,
        d: DeclId,
        before: NamedMarks,
    ) -> W<(u32, Payload)> {
        let n = before.marks.decls as u32;
        let mut named: Vec<(u32, DeclId)> =
            self.recs.iter().filter(|&(_, &r)| r >= n).map(|(&x, &r)| (r, x)).collect();
        named.sort_unstable();
        let mut redo = Vec::with_capacity(named.len());
        for &(r, x) in &named {
            let spec = match self.img.decls.get(r as usize).map(|x| &x.payload) {
                Some(Payload::Spec { .. }) => true,
                Some(Payload::Member { .. } | Payload::Implicit { .. }) => false,
                _ => {
                    return Err(USER);
                }
            };
            redo.push((x, spec));
        }
        if redo.is_empty() {
            return Err(USER);
        }
        self.rollback(before.marks);
        let NamedMarks { param_pos, req_recs, req_params, .. } = before;
        self.bs.param_pos.truncate(param_pos);
        self.bs.params.retain(|_, g| (*g as usize) < param_pos);
        self.bs.bspec.retain(|_, (c, _)| *c < n);
        self.img.reqs.recs.truncate(req_recs);
        self.img.reqs.nparams = req_params;
        self.bs.req.truncate(req_recs as u32, req_params as u32);
        for (x, spec) in redo {
            if self.recs.contains_key(&x) {
                continue;
            }
            if spec {
                self.spec_rec(x)?;
            } else {
                self.member_rec(x)?;
            }
        }
        let i = self.img.decls.len() as u32;
        let payload = self.record(d, i)?;
        if self.img.decls.len() as u32 != i {
            return Err(USER);
        }
        Ok((i, payload))
    }
}
