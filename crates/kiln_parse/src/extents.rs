//! The parser side of the declaration-extent table (spec/ast.md §19,
//! WP-2090).
//!
//! A declaration's first token is taken where the `declaration` (or
//! member-declaration, block declaration, parameter, ...) begins: the
//! outermost construct that starts it notes it as the pending lead
//! ([`Parser::ext_lead`]), a prefix construct (a template head, a
//! braceless `extern "C"` or `export`, `__extension__`) keeps an earlier
//! lead for the declaration nested in it, and the construct that makes
//! the declaration takes it ([`Parser::ext_begin`]). Each row is built
//! piecewise where its parts are known (the name at the declarator-id,
//! the body at its braces, the own range at the end of the
//! init-declarator) and completed at the declaration's last token
//! ([`Parser::ext_finish`]). Every helper is one branch when recording
//! is off; the lead bookkeeping (a `Loc` move) runs regardless.

use kiln_ast::DeclId;
use kiln_ast::decl_extents::{DeclEnd, DeclExtent};
use kiln_lex::{Punct, TokKind};
use kiln_sema::declspec::{DeclSpec, Declarator};
use kiln_source::{Loc, Span};

use crate::parser::Parser;

impl Parser<'_, '_> {
    /// True when the table records.
    #[inline]
    pub(crate) fn ext_on(&self) -> bool {
        self.s.ast.decl_extents.is_on()
    }

    /// Notes the current token as the first of the declaration about to
    /// be parsed, unless an enclosing prefix construct noted one.
    #[inline]
    pub(crate) fn ext_lead(&mut self) {
        if self.ext_lead.is_none() {
            self.ext_lead = Some(self.loc());
        }
    }

    /// The first token of the declaration starting here: the pending
    /// lead (taken), or the current token.
    #[inline]
    pub(crate) fn ext_begin(&mut self) -> Loc {
        self.ext_lead.take().unwrap_or_else(|| self.loc())
    }

    /// Drops a lead no declaration took (a construct that declares
    /// nothing, an error).
    #[inline]
    pub(crate) fn ext_drop_lead(&mut self) {
        self.ext_lead = None;
    }

    /// The end of the last token consumed and what it is.
    pub(crate) fn ext_last(&self) -> (Loc, DeclEnd) {
        let end = self.prev_end();
        let kind = match self.pos().checked_sub(1).map(|i| self.tok_at(i).kind) {
            Some(TokKind::Punct(Punct::Semi)) => DeclEnd::Semi,
            Some(TokKind::Punct(Punct::RBrace)) => DeclEnd::Brace,
            _ => DeclEnd::Other,
        };
        (end, kind)
    }

    /// `d`'s row (created when missing, and shared with the template `d`
    /// is the pattern of), when recording.
    pub(crate) fn ext_row(&mut self, d: DeclId) -> Option<&mut DeclExtent> {
        if !self.ext_on() {
            return None;
        }
        if self.s.ast.decl_extents.get(d).is_none() {
            let _ = self.s.ast.decl_extents.entry(d);
            if let Some(t) = self.s.ast.described_template(d) {
                self.s.ast.decl_extents.share(t, d);
            }
        }
        self.s.ast.decl_extents.get_mut(d)
    }

    /// Completes the rows of `decls`, one declaration that began at
    /// `begin` and ends at the last token consumed: `full`, `end`, and
    /// `own` where no narrower part was recorded.
    pub(crate) fn ext_finish(&mut self, decls: &[DeclId], begin: Loc) {
        if !self.ext_on() || decls.is_empty() {
            return;
        }
        let (hi, end) = self.ext_last();
        let full = Span::new(begin, hi);
        for &d in decls {
            if let Some(e) = self.ext_row(d) {
                e.full = full;
                e.end = end;
                if !e.own.lo.is_valid() {
                    e.own = full;
                }
            }
        }
    }

    /// [`Self::ext_finish`] for a declaration that ends without a `;`
    /// or `}` of its own (a parameter, a condition, an enumerator, a
    /// template parameter): its last token is never either, even when
    /// it is the `}` of an initializer.
    pub(crate) fn ext_finish_inner(&mut self, d: Option<DeclId>, begin: Loc) {
        let Some(d) = d else { return };
        self.ext_finish(&[d], begin);
        if let Some(e) = self.ext_row(d) {
            e.end = DeclEnd::Other;
        }
    }

    /// [`Self::ext_finish`] for one declaration.
    #[inline]
    pub(crate) fn ext_finish_one(&mut self, d: Option<DeclId>, begin: Loc) {
        if let Some(d) = d {
            self.ext_finish(&[d], begin);
        }
    }

    /// The declarator part of declarator `decl`'s entity `d`: its name,
    /// its own range from the declarator's start to the last token
    /// consumed, and the end of the decl-specifier-seq `specs_end`.
    pub(crate) fn ext_declarator(&mut self, d: Option<DeclId>, decl: &Declarator, specs_end: Loc) {
        let Some(d) = d else { return };
        if !self.ext_on() {
            return;
        }
        let own = Span::new(decl.loc, self.prev_end());
        let (name, qual) = (decl.id_span, decl.id_qual);
        if let Some(e) = self.ext_row(d) {
            // A `= default;` definition's own part ends with its body,
            // before the `;`.
            e.own = if e.body.lo.is_valid() { Span::new(own.lo, e.body.hi) } else { own };
            e.name = name;
            e.qual = qual;
            e.specs_end = specs_end;
        }
    }

    /// The declarator part of member declarator `decl` and of the
    /// function `target` it declares or befriends (when another decl),
    /// each pushed to `out`.
    pub(crate) fn ext_member(
        &mut self,
        decl: Option<DeclId>,
        target: Option<DeclId>,
        d: &Declarator,
        specs_end: Loc,
        out: &mut Vec<DeclId>,
    ) {
        for x in [decl, target.filter(|t| Some(*t) != decl)].into_iter().flatten() {
            if let Some(b) = target.and_then(|t| self.s.ast.decl_extents.get(t)).map(|e| e.body)
                && b.lo.is_valid()
                && let Some(e) = self.ext_row(x)
            {
                e.body = b;
            }
            self.ext_declarator(Some(x), d, specs_end);
            out.push(x);
        }
    }

    /// [`Self::ext_member`] and [`Self::ext_finish`] for a member function
    /// definition that began at `begin`.
    pub(crate) fn ext_member_finish(
        &mut self,
        decl: Option<DeclId>,
        target: Option<DeclId>,
        d: &Declarator,
        (begin, specs_end): (Loc, Loc),
    ) {
        if !self.ext_on() {
            return;
        }
        let mut out = Vec::new();
        self.ext_member(decl, target, d, specs_end, &mut out);
        self.ext_finish(&out, begin);
    }

    /// The body of `d`, from `lo` to the last token consumed.
    pub(crate) fn ext_body(&mut self, d: Option<DeclId>, lo: Loc) {
        let Some(d) = d else { return };
        if !self.ext_on() {
            return;
        }
        let body = Span::new(lo, self.prev_end());
        if let Some(e) = self.ext_row(d) {
            e.body = body;
        }
    }

    /// The body of `d`, `body` (a late-parsed body's saved tokens).
    pub(crate) fn ext_set_body(&mut self, d: Option<DeclId>, body: Span) {
        if let Some(d) = d
            && let Some(e) = self.ext_row(d)
        {
            e.body = body;
        }
    }

    /// The end of a decl-specifier-seq that began at token index `start`:
    /// the end of the last token consumed, or invalid when it took none.
    #[inline]
    pub(crate) fn ext_specs_end(&self, start: usize) -> Loc {
        if self.pos() > start { self.prev_end() } else { Loc::INVALID }
    }

    /// The name of `d`, `name`, after a qualifier starting at `qual`
    /// (invalid when unqualified).
    pub(crate) fn ext_name_span(&mut self, d: DeclId, qual: Loc, name: Span) {
        if let Some(e) = self.ext_row(d) {
            e.name = name;
            e.qual = qual;
        }
    }

    /// The name of `d` when it is an identifier at `d`'s location.
    pub(crate) fn ext_ident_name(&mut self, d: DeclId) {
        let n = self.s.ast.decls.get(d);
        let (loc, name) = (n.loc(), n.name());
        let Some(sym) = name.as_ident() else { return };
        let Ok(len) = u32::try_from(self.s.ast.symbols().as_str(sym).len()) else { return };
        if len == 0 || !loc.is_valid() {
            return;
        }
        if let Some(e) = self.ext_row(d) {
            e.name = Span::new(loc, loc.with_offset(len));
        }
    }

    /// The bindings of structured binding declaration `v` (when it is
    /// one): each its name, as its name and own range.
    pub(crate) fn ext_bindings(&mut self, v: DeclId) {
        if !self.ext_on() {
            return;
        }
        let Some(list) = self.s.ast.decls.decomp_bindings(v) else { return };
        let bindings: Vec<DeclId> = self.s.ast.lists.iter::<DeclId>(list).collect();
        for b in bindings {
            let n = self.s.ast.decls.get(b);
            let (loc, name) = (n.loc(), n.name());
            let len = name.as_ident().map_or(0, |s| self.s.ast.symbols().as_str(s).len());
            let len = u32::try_from(len).unwrap_or(0);
            let name = Span::new(loc, loc.with_offset(len));
            if let Some(e) = self.ext_row(b) {
                e.name = name;
                e.own = name;
            }
        }
    }

    /// `group` with the bindings of its structured binding declarations
    /// after each.
    pub(crate) fn ext_with_bindings(&self, group: Vec<DeclId>) -> Vec<DeclId> {
        if !self.ext_on() {
            return group;
        }
        let mut out = Vec::with_capacity(group.len());
        for d in group {
            out.push(d);
            if let Some(list) = self.s.ast.decls.decomp_bindings(d) {
                out.extend(self.s.ast.lists.iter::<DeclId>(list));
            }
        }
        out
    }

    /// The head of tag `d`'s specifier at `kw` (its class-key or `enum`):
    /// its own range starts there, with its name after qualifier `qual`.
    pub(crate) fn ext_tag_head(&mut self, d: DeclId, kw: Loc, qual: Loc, name: Span) {
        if let Some(e) = self.ext_row(d) {
            e.own = Span::new(kw, Loc::INVALID);
            e.name = name;
            e.qual = qual;
        }
    }

    /// The end of the tag specifier at `kw` just parsed into `ds`: its
    /// own range ends here, and is its full range until a declaration
    /// containing it completes the row.
    pub(crate) fn ext_tag_specifier_end(&mut self, ds: &DeclSpec, kw: Loc) {
        if !ds.tag_extent {
            return;
        }
        let Some(t) = ds.tag else { return };
        let hi = self.prev_end();
        if let Some(e) = self.ext_row(t) {
            e.own = Span::new(kw, hi);
            if !e.full.lo.is_valid() {
                e.full = e.own;
            }
        }
    }

    /// The decls of a declaration whose decl-specifier-seq is `ds`: the
    /// tag its own specifier declared ([`DeclSpec::tag_extent`]) first,
    /// then `decls`.
    pub(crate) fn ext_group(ds: &DeclSpec, decls: &[DeclId]) -> Vec<DeclId> {
        let mut out = Vec::with_capacity(decls.len() + 1);
        if ds.tag_extent
            && let Some(t) = ds.tag
        {
            out.push(t);
        }
        out.extend(decls.iter().copied().filter(|d| Some(*d) != ds.tag || !ds.tag_extent));
        out
    }
}
