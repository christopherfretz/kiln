//! Symbol visibility attributes and `#pragma GCC visibility`
//! (spec/sema.md §4.5 `visibility`, spec/codegen.md "Symbol visibility";
//! clang `handleVisibilityAttr`, `mergeVisibilityAttr`,
//! `checkNewAttributesAfterDef`, `AddPushedVisibilityAttribute`).
//!
//! Visibility is not part of the C++ standard ([basic.link] defines
//! linkage only); the GNU `visibility("default|hidden|internal|protected")`
//! attribute and clang's `type_visibility` record the ELF symbol
//! visibility codegen gives an entity. `act_on_attribute` keeps the
//! argument as written ([`Sema::visibility_attribute`]);
//! [`Sema::check_visibility_attrs`] checks the subject and the value,
//! replacing each node by one whose `a` is the visibility (0 default,
//! 1 hidden, 2 protected; `internal` is hidden, as clang maps it).
//! A `#pragma GCC visibility push(V)` adds an implicit attribute
//! ([`AttrNode::IMPLICIT`]) to each externally visible namespace-scope
//! function and variable and to each class declared while it is in
//! effect; a namespace with a visibility attribute suspends the pragma
//! inside its body (clang's `NoVisibility` stack marker).

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::DeclKind;
use kiln_ast::ids::RawId;
use kiln_ast::{AttrId, DeclId, ExprId};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::decl::AttrArg;

/// The `a` word of a visibility node whose argument is not checked yet
/// (`b` holds the argument expression).
const UNCHECKED: u32 = u32::MAX;

impl Sema<'_, '_> {
    /// `visibility(S)` / `type_visibility(S)` as written: exactly one
    /// argument (clang's generic argument-count check), an expression
    /// kept unchecked in `b`.
    pub(crate) fn visibility_attribute(
        &mut self,
        kind: AttrKind,
        syntax: AttrSyntax,
        shown: &str,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let name = if kind == AttrKind::TYPE_VISIBILITY { "type_visibility" } else { "visibility" };
        let arg = match args {
            [AttrArg::Expr(e)] => *e,
            [AttrArg::Ident(_, l)] => {
                let msg = self.diag(DiagId::parse_attribute_requires_string, *l).arg(name);
                self.emit(msg);
                return None;
            }
            [AttrArg::Type(_)] => {
                let msg = self.diag(DiagId::parse_attribute_requires_string, loc).arg(name);
                self.emit(msg);
                return None;
            }
            _ => {
                // clang 22 names a `[[gnu::x]]` attribute with its scope.
                let shown = if syntax == AttrSyntax::Cxx11 && self.clang_compat >= 22 {
                    shown.to_owned()
                } else {
                    name.to_owned()
                };
                let msg =
                    self.diag(DiagId::parse_attribute_wrong_arg_count, loc).arg(shown).arg("1");
                self.emit(msg);
                return None;
            }
        };
        let node = AttrNode { kind, syntax, flags: 0, loc, a: UNCHECKED, b: arg.raw() };
        Some(self.ast.attrs.push(node))
    }

    /// Checks the visibility attributes among `attrs` of `d` in clang's
    /// order (`handleVisibilityAttr`): ignored on a typedef,
    /// `type_visibility` only on a class, enumeration or namespace, a
    /// string literal naming a known visibility; a second attribute of
    /// the same kind with another value on the same declaration is
    /// `err_mismatched_visibility` (the later one is kept).
    pub(crate) fn check_visibility_attrs(&mut self, d: DeclId, attrs: &mut Vec<AttrId>) {
        if !attrs.iter().any(|&a| is_vis(self.ast.attrs.get(a).kind)) {
            return;
        }
        let kind = self.ast.decls.get(d).kind();
        let mut out = Vec::with_capacity(attrs.len());
        for &a in attrs.iter() {
            let node = *self.ast.attrs.get(a);
            if !is_vis(node.kind) || node.a != UNCHECKED {
                out.push(a);
                continue;
            }
            let Some(v) = self.visibility_value(kind, node) else { continue };
            let checked = self.ast.attrs.push(AttrNode { a: v, b: 0, ..node });
            // clang `mergeVisibilityAttr` on the same declaration.
            if let Some(i) = out.iter().position(|&o| self.ast.attrs.get(o).kind == node.kind) {
                let old = *self.ast.attrs.get(out[i]);
                if old.a == v {
                    continue;
                }
                let prev = self.diag(DiagId::note_previous_attribute, node.loc);
                let msg = self.diag(DiagId::sema_mismatched_visibility, old.loc).with_note(prev);
                self.emit(msg);
                out.remove(i);
            }
            out.push(checked);
        }
        *attrs = out;
    }

    /// The visibility of attribute `node` on a declaration of kind
    /// `kind`, or `None` after a diagnostic.
    fn visibility_value(&mut self, kind: DeclKind, node: AttrNode) -> Option<u32> {
        let type_vis = node.kind == AttrKind::TYPE_VISIBILITY;
        let shown = self.attr_shown(&node, if type_vis { "type_visibility" } else { "visibility" });
        if matches!(kind, DeclKind::Typedef | DeclKind::TypeAlias) {
            let msg = self.diag(DiagId::parse_attribute_ignored, node.loc).arg(shown);
            self.emit(msg);
            return None;
        }
        if type_vis && !(is_record(kind) || matches!(kind, DeclKind::Enum | DeclKind::Namespace)) {
            let msg = self
                .diag(DiagId::sema_attribute_wrong_decl_type_error, node.loc)
                .arg(format!("'{shown}'"))
                .arg("types and namespaces");
            self.emit(msg);
            return None;
        }
        let e = ExprId::from_raw(node.b)?;
        let Some(lit) = self.string_lit_of(e) else {
            let name = if type_vis { "type_visibility" } else { "visibility" };
            let msg = self
                .diag(DiagId::parse_attribute_requires_string, self.ast.exprs.get(e).loc())
                .arg(name);
            self.emit(msg);
            return None;
        };
        let id = self.ast.exprs.string_lit(lit);
        let text = String::from_utf8_lossy(self.ast.strings.bytes(id)).into_owned();
        match text.as_str() {
            "default" => Some(0),
            "hidden" | "internal" => Some(1),
            "protected" => Some(2),
            _ => {
                let msg = self
                    .diag(DiagId::sema_attribute_arg_not_supported, self.ast.exprs.get(lit).loc())
                    .arg(format!("'{shown}'"))
                    .arg(text);
                self.emit(msg);
                None
            }
        }
    }

    /// The visibility attributes of declaration `decl` against its
    /// previous declarations (clang `mergeDeclAttributes`), then the
    /// `#pragma GCC visibility` in effect: an attribute after the
    /// definition when the definition has none is ignored with a warning
    /// (`warn_attribute_precede_definition`); one that differs from a
    /// previous declaration's is `err_mismatched_visibility` and the
    /// previous one is kept. A namespace-scope function or variable with
    /// external linkage, or a class (`tag`), that has no visibility
    /// attribute here or on a previous declaration takes the pragma's
    /// as an implicit attribute.
    pub(crate) fn finish_visibility_attrs(
        &mut self,
        decl: DeclId,
        attrs: &mut Vec<AttrId>,
        tag: bool,
    ) {
        let own = attrs.iter().position(|&a| self.ast.attrs.get(a).kind == AttrKind::VISIBILITY);
        // Without an attribute of its own and without a pragma in effect
        // nothing below applies; the previous declarations are not walked
        // (a run of redeclarations would be quadratic, WP-1076).
        if own.is_none() && !matches!(self.vis_stack.last(), Some(Some(_))) {
            return;
        }
        let prior = self.prior_visibility_attr(decl);
        if let Some(i) = own {
            let node = *self.ast.attrs.get(attrs[i]);
            if let Some(def) = self.ast.definition(decl)
                && def != decl
                && prior.is_none()
                && self.ast.redecls_of(decl).any(|r| r == def)
            {
                let prev =
                    self.diag(DiagId::note_previous_definition, self.ast.decls.get(def).loc());
                let msg =
                    self.diag(DiagId::sema_attribute_precede_definition, node.loc).with_note(prev);
                self.emit(msg);
                attrs.remove(i);
            } else if let Some(p) = prior
                && p.a != node.a
            {
                let prev = self.diag(DiagId::note_previous_attribute, p.loc);
                let msg = self.diag(DiagId::sema_mismatched_visibility, node.loc).with_note(prev);
                self.emit(msg);
                attrs.remove(i);
            }
            return;
        }
        // A folded class redeclaration (`struct S;` naming the class) may
        // already carry the attribute.
        let attached = || {
            self.ast
                .lists
                .iter::<AttrId>(self.ast.attrs.of_decl(decl))
                .any(|a| self.ast.attrs.get(a).kind == AttrKind::VISIBILITY)
        };
        if prior.is_some() || attached() {
            return;
        }
        let Some(&Some((v, loc))) = self.vis_stack.last() else { return };
        if !tag && !self.pragma_visibility_applies(decl) {
            return;
        }
        let node = AttrNode {
            kind: AttrKind::VISIBILITY,
            syntax: AttrSyntax::Pragma,
            flags: AttrNode::IMPLICIT,
            loc,
            a: v,
            b: 0,
        };
        attrs.push(self.ast.attrs.push(node));
    }

    /// clang `ActOnExplicitInstantiation` (`ProcessDeclAttributeList`
    /// on the specialization): the `visibility` / `type_visibility`
    /// attributes of an explicit instantiation of a function or variable
    /// template specialization `spec` are the specialization's own
    /// (libc++'s `extern template _LIBCPP_EXPORTED_FROM_ABI string
    /// operator+<char>(...)` exports a hidden template's specialization).
    /// One that differs from an earlier explicit instantiation's replaces
    /// it after `err_mismatched_visibility` (clang `mergeVisibilityAttr`).
    pub(crate) fn explicit_inst_visibility_attrs(&mut self, spec: DeclId, attrs: &[AttrId]) {
        let mut vis: Vec<AttrId> =
            attrs.iter().copied().filter(|&a| is_vis(self.ast.attrs.get(a).kind)).collect();
        if vis.is_empty() {
            return;
        }
        self.check_visibility_attrs(spec, &mut vis);
        if vis.is_empty() {
            return;
        }
        let old: Vec<AttrId> = self.ast.lists.iter(self.ast.attrs.of_decl(spec)).collect();
        if old.is_empty() {
            self.builder().append_attrs(spec, &vis);
            return;
        }
        let mut keep = Vec::with_capacity(old.len() + vis.len());
        for a in old {
            let o = *self.ast.attrs.get(a);
            let new = vis.iter().position(|&n| self.ast.attrs.get(n).kind == o.kind);
            match new {
                Some(i) if self.ast.attrs.get(vis[i]).a == o.a => {
                    vis.remove(i);
                    keep.push(a);
                }
                Some(i) => {
                    let at = self.ast.attrs.get(vis[i]).loc;
                    let prev = self.diag(DiagId::note_previous_attribute, at);
                    let msg = self.diag(DiagId::sema_mismatched_visibility, o.loc).with_note(prev);
                    self.emit(msg);
                }
                None => keep.push(a),
            }
        }
        if vis.is_empty() {
            return;
        }
        keep.extend(vis);
        // `spec` already has an attribute list (and `HAS_ATTRS`).
        let list = self.ast.lists.push(&keep);
        let _ = self.ast.attrs.decl_attrs.refine(spec, list);
    }

    /// The visibility attribute of the nearest previous declaration of
    /// `decl` that has one.
    fn prior_visibility_attr(&self, decl: DeclId) -> Option<AttrNode> {
        self.ast.redecls_of(decl).filter(|&r| r != decl).find_map(|r| {
            self.ast
                .lists
                .iter::<AttrId>(self.ast.attrs.of_decl(r))
                .map(|a| *self.ast.attrs.get(a))
                .find(|n| n.kind == AttrKind::VISIBILITY)
        })
    }

    /// clang `ActOn{Function,Variable}Declarator`: the pragma applies to
    /// an externally visible function or variable not declared in a
    /// class.
    fn pragma_visibility_applies(&self, decl: DeclId) -> bool {
        let n = self.ast.decls.get(decl);
        let var = n.kind() == DeclKind::Var;
        if !(var || n.kind().is_function()) {
            return false;
        }
        // Not a member, nor a block-scope declaration (kiln does not
        // apply the pragma to a block-scope `extern` declaration).
        let member_or_local = n.ctx().is_some_and(|c| {
            let k = self.ast.decls.get(c).kind();
            is_record(k) || k.is_function()
        });
        if member_or_local {
            return false;
        }
        !kiln_abi::mangle::has_internal_linkage(&self.ast, decl)
    }

    /// `#pragma weak name` at `loc`, the name (clang `ActOnPragmaWeakID`):
    /// a function or variable `name` declared at namespace scope of the
    /// translation unit becomes weak (an implicit `weak` attribute; any
    /// other declaration found is left as it is); otherwise the pragma
    /// waits for the first `extern "C"` function or variable declared
    /// with that name ([`Sema::apply_pragma_weak`]), and is warned about at
    /// the end of the translation unit if there is none.
    pub fn act_on_pragma_weak(&mut self, name: kiln_source::symbol::Symbol, loc: Loc) {
        let found = self.lookup_in(self.ast.tu, name, crate::lookup::LookupKind::Ordinary);
        let Some(&d) = found.decls.first() else {
            self.weak_pending.push(crate::pragma_names::WeakPending { name, loc, alias: None });
            return;
        };
        let k = self.kind_of(d);
        if k == DeclKind::Var || k.is_function() {
            let a = self.ast.attrs.push(weak_pragma_attr(loc));
            self.builder().append_attrs(d, &[a]);
        }
    }

    /// `#pragma GCC visibility push(V)` (`Some(V)`, a valid name) or
    /// `pop` (`None`; clang `ActOnPragmaVisibility` /
    /// `PopPragmaVisibility`). A `pop` with nothing pushed, or whose
    /// innermost entry is a namespace's, is
    /// `err_pragma_pop_visibility_mismatch` (with the namespace's note)
    /// and pops nothing.
    pub fn act_on_pragma_visibility(&mut self, push: Option<&str>, loc: Loc) {
        match push {
            Some(v) => {
                let v = match v {
                    "default" => 0,
                    "protected" => 2,
                    _ => 1,
                };
                self.vis_stack.push(Some((v, loc)));
            }
            None => match self.vis_stack.last() {
                Some(Some(_)) => {
                    self.vis_stack.pop();
                }
                Some(None) => {
                    let ns = self.vis_namespaces.last().copied();
                    let mut d = self.diag(DiagId::sema_visibility_pop_mismatch, loc);
                    if let Some(ns) = ns {
                        let at = self.ast.decls.get(ns).loc();
                        d = d.with_note(
                            self.diag(DiagId::note_surrounding_namespace_starts_here, at),
                        );
                    }
                    self.emit(d);
                }
                None => {
                    let d = self.diag(DiagId::sema_visibility_pop_mismatch, loc);
                    self.emit(d);
                }
            },
        }
    }

    /// Namespace definition `ns` with visibility attribute: the pragma
    /// stack is suspended in its body (clang `PushNamespaceVisibilityAttr`).
    pub(crate) fn push_namespace_visibility(&mut self, ns: DeclId) {
        self.vis_stack.push(None);
        self.vis_namespaces.push(ns);
    }

    /// The end (`}` at `end`) of namespace definition `ns`: pops its
    /// marker. A pragma pushed inside and not popped is clang's
    /// `err_pragma_push_visibility_mismatch` at the innermost push, with
    /// the note at `end`, and is popped too (clang's recovery). A no-op
    /// when `ns` has no marker or it is already popped.
    pub fn pop_namespace_visibility(&mut self, ns: DeclId, end: Loc) {
        if self.vis_namespaces.last() != Some(&ns) {
            return;
        }
        self.vis_namespaces.pop();
        if let Some(Some((_, at))) = self.vis_stack.last() {
            let note = self.diag(DiagId::note_surrounding_namespace_ends_here, end);
            let d = self.diag(DiagId::sema_visibility_push_mismatch, *at).with_note(note);
            self.emit(d);
        }
        while let Some(top) = self.vis_stack.pop() {
            if top.is_none() {
                break;
            }
        }
    }
}

fn is_vis(k: AttrKind) -> bool {
    k == AttrKind::VISIBILITY || k == AttrKind::TYPE_VISIBILITY
}

fn is_record(k: DeclKind) -> bool {
    matches!(
        k,
        DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization
    )
}

/// The implicit `weak` attribute a `#pragma weak` at `loc` gives.
pub(crate) fn weak_pragma_attr(loc: Loc) -> AttrNode {
    AttrNode {
        kind: AttrKind::WEAK,
        syntax: AttrSyntax::Pragma,
        flags: AttrNode::IMPLICIT,
        loc,
        a: 0,
        b: 0,
    }
}
