//! clang's generic "attribute declaration must precede definition"
//! check (`Sema::checkNewAttributesAfterDef`, spec/sema.md §4.5;
//! WP-1807): an attribute that a redeclaration adds after its entity's
//! definition is ignored with a `-Wignored-attributes` warning noting the
//! definition, unless the definition (or a declaration before it) already
//! has an attribute of the same kind. The attributes with rules of their
//! own after a definition (`alias` / `ifunc`, `alignas`, `visibility`,
//! `internal_linkage`, `weak_import`, `selectany`, `ms_struct` /
//! `gcc_struct`, the function multiversioning attributes, and
//! `require_constant_initialization`) are checked where they are handled;
//! the type attributes kiln keeps on a declaration (GNU `noreturn` and
//! `nothrow`, the vector and mode attributes, calling conventions) are
//! not clang declaration attributes and are never checked.

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::DeclKind;
use kiln_ast::{AttrId, DeclId};
use kiln_diag::DiagId;

use crate::Sema;

/// The clang attribute class of a kiln kind, for "the definition already
/// has one" (clang's `UnusedAttr` is both `unused` and `maybe_unused`).
fn attr_class(k: AttrKind) -> AttrKind {
    if k == AttrKind::MAYBE_UNUSED { AttrKind::UNUSED } else { k }
}

impl Sema<'_, '_> {
    /// clang `checkNewAttributesAfterDef` for the declaration `d` with
    /// its own attributes `attrs` (after their subject checks, before
    /// they are attached): each one added after the entity's definition
    /// is reported and removed. `folded`: `d` is a class or enumeration
    /// redeclaration kiln folds into its definition `d` itself (`enum E;`
    /// after `enum E { }`).
    pub(crate) fn check_new_attrs_after_def(
        &mut self,
        d: DeclId,
        attrs: &mut Vec<AttrId>,
        folded: bool,
    ) {
        if attrs.is_empty() {
            return;
        }
        let def = if folded { Some(d) } else { self.prior_definition(d) };
        let Some(def) = def else { return };
        let mut i = 0;
        while i < attrs.len() {
            let node = *self.ast.attrs.get(attrs[i]);
            if self.attr_allowed_after_def(&node) || self.def_has_attr_class(def, node.kind) {
                i += 1;
                continue;
            }
            let prev = self.diag(DiagId::note_previous_definition, self.ast.decls.get(def).loc());
            let msg =
                self.diag(DiagId::sema_attribute_precede_definition, node.loc).with_note(prev);
            self.emit(msg);
            attrs.remove(i);
        }
    }

    /// The declaration whose attributes declaration `d`'s are merged with
    /// (clang's `Old` in `mergeDeclAttributes`): its previous declaration,
    /// or for a redeclaration an error left out of the chain, the one it
    /// would have followed, and none for one clang rejects before merging
    /// (`Sema::after_def_prev`, WP-1840).
    pub(crate) fn attr_merge_prev(&self, d: DeclId) -> Option<DeclId> {
        match self.after_def_prev {
            Some((n, p)) if n == d => p,
            _ => self.ast.prev(d),
        }
    }

    /// The definition of `d`'s entity that an earlier declaration is
    /// (clang `getDefinition` of the previous declaration): a function's
    /// body (or `= delete` / `= default`), a variable definition (in C
    /// one with an initializer: a tentative definition only matters to
    /// `alias`), a class or enumeration's. None for every other kind, for
    /// an explicit or partial specialization of a class template (clang
    /// merges no attributes into one), and when `d` itself is the
    /// definition. An invalid redeclaration out of the chain is checked
    /// against the declaration it would have followed (an `inline` or
    /// `constexpr` mismatch after a definition, a variable redefinition or
    /// conflicting redeclaration), and a `static` function redeclaration of
    /// a non-static one against none (WP-1840).
    fn prior_definition(&self, d: DeclId) -> Option<DeclId> {
        let prev = self.attr_merge_prev(d)?;
        let kind = self.kind_of(d);
        if kind.is_function() {
            // A deleted or defaulted function is defined too.
            let defined = |p: DeclId| {
                self.ast.decls.function(p).is_some_and(|f| {
                    f.spec.is_deleted() || f.spec.defaulted() != kiln_ast::decl::Defaulted::No
                })
            };
            return self.ast.definition(prev).filter(|&f| f != d).or_else(|| {
                std::iter::successors(Some(prev), |&p| self.ast.prev(p))
                    .take(4096)
                    .find(|&p| defined(p))
            });
        }
        match kind {
            DeclKind::Var => {
                let c = self.is_c();
                std::iter::successors(Some(prev), |&p| self.ast.prev(p)).find(|&p| {
                    if c {
                        self.ast.decls.var(p).is_some_and(|v| v.init.is_some())
                    } else {
                        self.is_var_definition(p)
                    }
                })
            }
            DeclKind::CXXRecord | DeclKind::Enum if self.ast.spec_info(d).is_none() => {
                self.ast.definition(prev).filter(|&r| r != d)
            }
            _ => None,
        }
    }

    /// Attribute `node` may be added after the definition (clang's
    /// exemptions in `checkNewAttributesAfterDef`), is checked by its own
    /// handler, or is not a clang declaration attribute.
    fn attr_allowed_after_def(&self, node: &AttrNode) -> bool {
        match node.kind {
            // Their own after-definition rules; `enable_if`, which makes
            // the declaration an overload of its own in clang; the type
            // attributes, the parameter ABIs, and annotations
            // ([dcl.attr.annotation]: every declaration's accumulate).
            AttrKind::ALIAS
            | AttrKind::IFUNC
            | AttrKind::VISIBILITY
            | AttrKind::INTERNAL_LINKAGE
            | AttrKind::WEAK_IMPORT
            | AttrKind::SELECTANY
            | AttrKind::MS_STRUCT
            | AttrKind::GCC_STRUCT
            | AttrKind::REQUIRE_CONSTANT_INIT
            | AttrKind::TARGET
            | AttrKind::TARGET_VERSION
            | AttrKind::TARGET_CLONES
            | AttrKind::CPU_DISPATCH
            | AttrKind::CPU_SPECIFIC
            | AttrKind::ENABLE_IF
            | AttrKind::NOTHROW
            | AttrKind::LIFETIMEBOUND
            | AttrKind::SWIFT_PARAM_ABI
            | AttrKind::ANNOTATION
            | AttrKind::SME_TYPE => true,
            // `alignas` is an error at the definition (align_redecl.rs);
            // GNU `aligned` is checked here.
            AttrKind::ALIGNED => node.syntax == AttrSyntax::Keyword,
            // GNU `noreturn` is a function type attribute and C11
            // `_Noreturn` is exempt; `[[noreturn]]` (clang
            // `CXX11NoReturnAttr`) is checked.
            AttrKind::NORETURN => node.is_gnu_spelling() || node.syntax == AttrSyntax::Keyword,
            // clang 22: [dcl.attr.deprecated]/3, [dcl.attr.unused]/4 and
            // [dcl.attr.nodiscard]/2 let a redeclaration add them, in the
            // standard syntax (any scope).
            AttrKind::DEPRECATED
            | AttrKind::NODISCARD
            | AttrKind::MAYBE_UNUSED
            | AttrKind::UNUSED => self.clang_compat >= 22 && node.syntax == AttrSyntax::Cxx11,
            k => k.is_vector_type() || k.is_fn_type_attr(),
        }
    }

    /// Whether definition `def` (or a declaration before it, whose
    /// attributes it inherits) has an attribute of `kind`'s clang class.
    fn def_has_attr_class(&self, def: DeclId, kind: AttrKind) -> bool {
        let class = attr_class(kind);
        std::iter::successors(Some(def), |&p| self.ast.prev(p)).take(4096).any(|x| {
            self.ast
                .lists
                .iter::<AttrId>(self.ast.attrs.of_decl(x))
                .any(|a| attr_class(self.ast.attrs.get(a).kind) == class)
        })
    }
}
