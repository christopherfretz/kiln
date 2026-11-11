//! GNU `transparent_union` in C++ (clang `TransparentUnionAttr`; GCC
//! "Common Type Attributes"; WP-809).
//!
//! The attribute is C-only (`LangOpts = [COnly]`), so clang ignores every
//! occurrence in C++ with `'transparent_union' attribute ignored`. But the
//! occurrences after the class-key of a tag also reach clang's delayed
//! handler (`ProcessDeclAttributeDelayed` -> `handleTransparentUnionAttr`),
//! which does not check the language: it diagnoses a non-union subject
//! (including a class template), a non-definition, and a union whose
//! fields do not qualify, and otherwise keeps the attribute. A kept
//! attribute makes no argument conversions in C++ (overload resolution
//! never looks at it), but the calling convention passes such a union by
//! value as its first field (clang `useFirstFieldIfTransparentUnion`,
//! `kiln_abi::call`).
//!
//! In C (WP-1287) the attribute is clang's `handleTransparentUnionAttr`
//! everywhere: kept on a union definition (written after the class-key,
//! after the closing brace, or on a typedef of a complete union, glibc's
//! `__SOCKADDR_ARG`) whose fields qualify, and an argument passed to a
//! parameter of such a union converts to the first member it is
//! assignment-compatible with ([`Sema::transparent_union_argument`]).

use kiln_ast::attr::{AttrKind, AttrNode};
use kiln_ast::decl::{DeclKind, DefinitionBits, TagKind};
use kiln_ast::{AttrId, DeclId, ExprId, QualType};
use kiln_diag::DiagId;

use crate::Sema;
use crate::decl::TagUse;

impl Sema<'_, '_> {
    /// The `transparent_union` attributes among the class-head
    /// attributes `attrs` of tag `d`: each is ignored with a warning, then
    /// put through clang's delayed subject check. All are removed from
    /// `attrs`; those on a union definition are returned, to be attached
    /// for [`Sema::check_transparent_union`] at the closing brace.
    pub(crate) fn transparent_union_head(
        &mut self,
        d: DeclId,
        attrs: &mut Vec<AttrId>,
        use_: TagUse,
    ) -> Vec<AttrId> {
        let mut found = Vec::new();
        let mut kept = Vec::new();
        attrs.retain(|&a| {
            let keep = self.ast.attrs.get(a).kind != AttrKind::TRANSPARENT_UNION;
            if !keep {
                found.push(a);
            }
            keep
        });
        // clang's delayed handler only sees class-heads, not enum-heads.
        let enum_ = self.ast.decls.get(d).kind() == DeclKind::Enum;
        let union = self.ast.decls.get(d).kind() == DeclKind::CXXRecord
            && self.ast.decls.record(d).is_some_and(|r| r.tag == TagKind::Union)
            && self.ast.described_template(d).is_none();
        for a in found {
            let node = *self.ast.attrs.get(a);
            let shown = self.attr_shown(&node, "transparent_union");
            if !self.is_c() {
                let msg = self.diag(DiagId::parse_attribute_ignored, node.loc).arg(&shown);
                self.emit(msg);
                if enum_ {
                    continue;
                }
            }
            if !union {
                let msg = self
                    .diag(DiagId::sema_attribute_wrong_decl_type, node.loc)
                    .arg(format!("'{shown}'"))
                    .arg("unions");
                self.emit(msg);
            } else if use_ != TagUse::Definition {
                let msg = self.diag(DiagId::sema_transparent_union_not_definition, node.loc);
                self.emit(msg);
            } else {
                kept.push(a);
            }
        }
        kept
    }

    /// At the closing brace of union `r`: clang `handleTransparentUnionAttr`'s
    /// field checks for a kept class-head `transparent_union`, which is
    /// dropped (with a warning) when they fail. A field of incomplete or
    /// dependent type drops it silently, as in clang.
    pub(crate) fn check_transparent_union(&mut self, r: DeclId) {
        let Some(a) = self
            .ast
            .lists
            .iter::<AttrId>(self.ast.attrs.of_decl(r))
            .find(|&a| self.ast.attrs.get(a).kind == AttrKind::TRANSPARENT_UNION)
        else {
            return;
        };
        if !self.transparent_union_fields_ok(r, a) {
            self.builder().remove_attr(r, a);
        }
    }

    /// A `transparent_union` among the declaration attributes of `d` in C
    /// (clang `handleTransparentUnionAttr`): on a union being defined (its
    /// trailing attributes) it is kept for [`Sema::check_transparent_union`]
    /// at the closing brace; on a typedef of a complete union whose fields
    /// qualify it moves to the union; anything else drops it with a
    /// warning. True keeps it on `d`.
    pub(crate) fn c_transparent_union_attr(
        &mut self,
        d: DeclId,
        kind: DeclKind,
        a: AttrId,
        node: AttrNode,
    ) -> bool {
        let is_union =
            |s: &Self, r: DeclId| s.ast.decls.record(r).is_some_and(|x| x.tag == TagKind::Union);
        let typedef = matches!(kind, DeclKind::Typedef | DeclKind::TypeAlias);
        let rec = if typedef {
            self.ast
                .decls
                .typedef(d)
                .and_then(|t| self.ast.types.as_record(t.underlying))
                .filter(|&r| is_union(self, r))
        } else {
            (kind == DeclKind::CXXRecord && is_union(self, d)).then_some(d)
        };
        let Some(rec) = rec else {
            let shown = self.attr_shown(&node, "transparent_union");
            let msg = self
                .diag(DiagId::sema_attribute_wrong_decl_type, node.loc)
                .arg(format!("'{shown}'"))
                .arg("unions");
            self.emit(msg);
            return false;
        };
        if !typedef {
            return true;
        }
        let def = self.ast.definition(rec).filter(|&x| {
            self.ast.decls.record(x).is_some_and(|r| r.defn.contains(DefinitionBits::COMPLETE))
        });
        let Some(def) = def else {
            let msg = self.diag(DiagId::sema_transparent_union_not_definition, node.loc);
            self.emit(msg);
            return false;
        };
        if self.transparent_union_fields_ok(def, a) {
            self.builder().append_attrs(def, &[a]);
        }
        false
    }

    /// The union type of parameter type `to` when it kept
    /// `transparent_union` (C only).
    fn transparent_union_of(&self, to: QualType) -> Option<DeclId> {
        let r = self.ast.types.as_record(to)?;
        let def = self.ast.definition(r)?;
        self.ast
            .lists
            .iter::<AttrId>(self.ast.attrs.of_decl(def))
            .any(|a| self.ast.attrs.get(a).kind == AttrKind::TRANSPARENT_UNION)
            .then_some(def)
    }

    /// clang `mergeFunctionParameterTypes` (GNU, C only): in a
    /// redeclaration, a parameter of transparent union type is
    /// compatible with one whose type is compatible with an
    /// (unqualified) member of the union, and the merged parameter is
    /// that member's type (`mergeTransparentUnionType`). Returns the
    /// function types `pty` and `ty` with each such pair of parameters
    /// replaced by the member type, or `None` when no pair matches.
    pub(crate) fn c_transparent_param_merge(
        &mut self,
        pty: QualType,
        ty: QualType,
    ) -> Option<(QualType, QualType)> {
        let (a, b) = (self.ast.types.proto(pty).copied()?, self.ast.types.proto(ty).copied()?);
        let mut pa = self.ast.types.params(&a).to_vec();
        let mut pb = self.ast.types.params(&b).to_vec();
        if pa.len() != pb.len() {
            return None;
        }
        let mut changed = false;
        for (x, y) in pa.iter_mut().zip(pb.iter_mut()) {
            let m = self
                .transparent_member_match(*x, *y)
                .or_else(|| self.transparent_member_match(*y, *x));
            if let Some(m) = m {
                (*x, *y) = (m, m);
                changed = true;
            }
        }
        if !changed {
            return None;
        }
        let (ea, eb) = (self.ast.types.exc_spec(&a), self.ast.types.exc_spec(&b));
        let ta = self.ast.types.function(a.ret(), &pa, a.info(), ea);
        let tb = self.ast.types.function(b.ret(), &pb, b.info(), eb);
        Some((ta, tb))
    }

    /// The first unqualified member type of transparent union `u`
    /// compatible with `other`.
    fn transparent_member_match(&self, u: QualType, other: QualType) -> Option<QualType> {
        self.transparent_union_members(u)?
            .into_iter()
            .find(|&t| crate::conv::c_assign::compatible(self, t, other))
    }

    /// The unqualified member types of `u` when it is a union that kept
    /// `transparent_union`.
    pub(crate) fn transparent_union_members(&self, u: QualType) -> Option<Vec<QualType>> {
        let def = self.transparent_union_of(u)?;
        Some(
            self.ast
                .decls
                .members(def)
                .filter(|&m| self.ast.decls.get(m).kind() == DeclKind::Field)
                .filter_map(|m| self.ast.decls.get(m).ty())
                .map(|t| crate::conv::strip_cv(self, t))
                .collect(),
        )
    }

    /// clang `CheckTransparentUnionArgumentConstraints`: argument `e`
    /// passed to a parameter of transparent union type `to` (after the
    /// plain assignment check failed) initializes the first member that
    /// takes it: a pointer member a `void *` or a null pointer constant,
    /// any member a value compatible with it by assignment without a
    /// diagnostic. The result is the union prvalue (clang
    /// `ConstructTransparentUnion`); `None` when no member takes it.
    pub(crate) fn transparent_union_argument(&mut self, e: ExprId, to: QualType) -> Option<ExprId> {
        let def = self.transparent_union_of(to)?;
        let fields: Vec<DeclId> = self
            .ast
            .decls
            .members(def)
            .filter(|&m| self.ast.decls.get(m).kind() == DeclKind::Field)
            .collect();
        for f in fields {
            let Some(fty) = self.ast.decls.get(f).ty() else { continue };
            let Some(x) = crate::conv::c_assign::argument_member(self, e, fty) else {
                continue;
            };
            let loc = self.ast.exprs.get(e).loc();
            let end = self.expr_end_loc(e);
            let u = crate::conv::strip_cv(self, to);
            return Some(self.builder().init_list(u, &[x], None, Some(f), loc, end));
        }
        None
    }

    fn transparent_union_fields_ok(&mut self, r: DeclId, a: AttrId) -> bool {
        let fields: Vec<DeclId> = self
            .ast
            .decls
            .members(r)
            .filter(|&m| self.ast.decls.get(m).kind() == DeclKind::Field)
            .collect();
        let Some(&first) = fields.first() else {
            let msg =
                self.diag(DiagId::sema_transparent_union_zero_fields, self.ast.attrs.get(a).loc);
            self.emit(msg);
            return false;
        };
        let Some(first_ty) = self.ast.decls.get(first).ty() else { return false };
        let vector = self.is_vector_type(first_ty);
        let floating = self.ast.types.is_floating(first_ty)
            || matches!(self.ast.types.canonical_view(first_ty),
                kiln_ast::types::TypeView::Complex(e) if self.ast.types.is_floating(e));
        if floating || vector {
            let ty = self.ty_arg(first_ty);
            let msg = self
                .diag(DiagId::sema_transparent_union_floating, self.ast.decls.get(first).loc())
                .arg(u32::from(vector))
                .arg(ty);
            self.emit(msg);
            return false;
        }
        let Some((size, align)) = crate::target::size_align(&self.ast, first_ty, self.abi()) else {
            return false;
        };
        for &f in &fields {
            let Some(ty) = self.ast.decls.get(f).ty() else { return false };
            let Some((fs, fa)) = crate::target::size_align(&self.ast, ty, self.abi()) else {
                return false;
            };
            if fs != size || fa > align {
                let is_size = fs != size;
                let (bits, first_bits) = if is_size { (fs, size) } else { (fa, align) };
                let note = self
                    .diag(
                        DiagId::note_transparent_union_first_field,
                        self.ast.decls.get(first).loc(),
                    )
                    .arg(u32::from(is_size))
                    .arg(first_bits * 8);
                let name = self.decl_name(f);
                let msg = self
                    .diag(
                        DiagId::sema_transparent_union_field_mismatch,
                        self.ast.decls.get(f).loc(),
                    )
                    .arg(u32::from(is_size))
                    .arg(name)
                    .arg(bits * 8)
                    .with_note(note);
                self.emit(msg);
                return false;
            }
        }
        true
    }
}
