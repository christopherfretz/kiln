//! `[[gsl::Owner]]` / `[[gsl::Pointer]]` classes for the dangling
//! analysis (clang 19 `isRecordWithAttr`, `Sema::inferGslOwnerPointerAttribute`
//! and `Sema::inferGslPointerAttribute`; spec/sema-class.md §5.6).
//!
//! A class is an Owner or a Pointer by an explicit attribute on any of
//! its declarations, by clang's implicit annotations of the `std`
//! containers and views (a class directly in `std` named in
//! [`STD_OWNERS`] / [`STD_POINTERS`], or a class named `iterator` &c.
//! nested in a `std` container), or because a `std` container's
//! `iterator` typedef names it. The name rules apply where clang runs
//! them, on class definitions and class template patterns, not on
//! (partial or explicit) specializations: `std::vector<bool>` is neither.
//! An implicit instantiation takes its pattern's kind.

use kiln_ast::attr::AttrKind;
use kiln_ast::decl::{DeclKind, PayloadClass};
use kiln_ast::templates::{SpecKind, is_template_kind};
use kiln_ast::types::TypeView;
use kiln_ast::{AttrId, DeclId, QualType};

use crate::Sema;

/// A gsl class kind.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Gsl {
    /// `[[gsl::Owner]]`: owns what its Pointers point to.
    Owner,
    /// `[[gsl::Pointer]]`: refers to what an Owner owns.
    Pointer,
}

/// clang 19 `StdOwners`.
const STD_OWNERS: [&str; 22] = [
    "any",
    "array",
    "basic_regex",
    "basic_string",
    "deque",
    "forward_list",
    "vector",
    "list",
    "map",
    "multiset",
    "multimap",
    "optional",
    "priority_queue",
    "queue",
    "set",
    "stack",
    "unique_ptr",
    "unordered_set",
    "unordered_map",
    "unordered_multiset",
    "unordered_multimap",
    "variant",
];

/// clang 19 `StdPointers`.
const STD_POINTERS: [&str; 4] = ["basic_string_view", "reference_wrapper", "regex_iterator", "span"];

/// clang 19 `inferGslPointerAttribute`'s `Containers`.
const CONTAINERS: [&str; 17] = [
    "array",
    "basic_string",
    "deque",
    "forward_list",
    "vector",
    "list",
    "map",
    "multiset",
    "multimap",
    "priority_queue",
    "queue",
    "set",
    "stack",
    "unordered_set",
    "unordered_map",
    "unordered_multiset",
    "unordered_multimap",
];

/// clang 19 `inferGslPointerAttribute`'s `Iterators`.
const ITERATORS: [&str; 4] =
    ["iterator", "const_iterator", "reverse_iterator", "const_reverse_iterator"];

/// A class (or class template specialization) kind.
pub(crate) fn is_record(k: DeclKind) -> bool {
    k.payload() == PayloadClass::Record
}

impl Sema<'_, '_> {
    /// The gsl kind of type `ty`: a class type's (not through a
    /// reference; clang `isRecordWithAttr`).
    pub(crate) fn type_gsl_kind(&self, ty: QualType) -> Option<Gsl> {
        self.ast.types.as_record(ty).and_then(|r| self.gsl_kind(r))
    }

    /// The gsl kind of class `r`.
    pub(crate) fn gsl_kind(&self, r: DeclId) -> Option<Gsl> {
        let mut cur = r;
        for _ in 0..64 {
            if let Some(k) = self.gsl_own_kind(cur) {
                return Some(k);
            }
            if self.kind_of(cur) == DeclKind::ClassTemplatePartialSpecialization
                || self.ast.partial_spec_info(cur).is_some()
            {
                return None;
            }
            if let Some((_, s)) = self.ast.spec_info(cur) {
                if s.kind == SpecKind::ExplicitSpecialization {
                    return None;
                }
                let from = s.from;
                let from_kind = self.kind_of(from);
                let next = if from_kind != DeclKind::ClassTemplatePartialSpecialization
                    && is_template_kind(from_kind)
                {
                    self.ast.template_info(from).and_then(|t| t.pattern)
                } else {
                    Some(from)
                };
                match next {
                    Some(n) if n != cur => cur = n,
                    _ => return None,
                }
                continue;
            }
            match self.ast.instantiated_from(cur) {
                Some(p) if p != cur => cur = p,
                _ => return None,
            }
        }
        None
    }

    /// The kind `d` itself has: an attribute on one of its declarations,
    /// clang's name rules (a class or class template pattern), or an
    /// `iterator` typedef inference.
    fn gsl_own_kind(&self, d: DeclId) -> Option<Gsl> {
        for x in self.ast.redecls_of(d) {
            for a in self.ast.lists.iter::<AttrId>(self.ast.attrs.of_decl(x)) {
                match self.ast.attrs.get(a).kind {
                    AttrKind::GSL_OWNER => return Some(Gsl::Owner),
                    AttrKind::GSL_POINTER => return Some(Gsl::Pointer),
                    _ => {}
                }
            }
        }
        if self.kind_of(d) == DeclKind::CXXRecord
            && self.ast.spec_info(d).is_none()
            && self.ast.partial_spec_info(d).is_none()
            && let Some(name) = self.ast.decls.get(d).name().as_ident()
        {
            let name = self.ast.symbols().as_str(name);
            if self.is_in_std_namespace(d) {
                if STD_OWNERS.contains(&name) {
                    return Some(Gsl::Owner);
                }
                if STD_POINTERS.contains(&name) {
                    return Some(Gsl::Pointer);
                }
            } else if self.names_std_container_iterator(d, name) {
                return Some(Gsl::Pointer);
            }
        }
        self.gsl_inferred_pointers.contains(&self.ast.first(d)).then_some(Gsl::Pointer)
    }

    /// Whether `d`, named `name`, is a member of a `std` container named
    /// like one of its iterators.
    fn names_std_container_iterator(&self, d: DeclId, name: &str) -> bool {
        if !ITERATORS.contains(&name) {
            return false;
        }
        let Some(parent) = self.ast.decls.get(d).ctx() else { return false };
        if !is_record(self.kind_of(parent)) || !self.is_in_std_namespace(parent) {
            return false;
        }
        self.ast
            .decls
            .get(parent)
            .name()
            .as_ident()
            .is_some_and(|p| CONTAINERS.contains(&self.ast.symbols().as_str(p)))
    }

    /// clang `inferGslPointerAttribute(TypedefNameDecl *)` for the new
    /// typedef `td`: a `std` container's `iterator` typedef makes the
    /// class it names (or the pattern of the template it specializes) a
    /// Pointer unless it has a gsl attribute.
    pub(crate) fn infer_gsl_typedef(&mut self, td: DeclId) {
        let Some(name) = self.ast.decls.get(td).name().as_ident() else { return };
        let name = self.ast.symbols().as_str(name).to_owned();
        if !self.names_std_container_iterator(td, &name) {
            return;
        }
        let Some(u) = self.ast.decls.typedef(td).map(|x| x.underlying) else { return };
        let c = self.ast.types.canonical(u);
        let record = match self.ast.types.view(c.ty()) {
            TypeView::Record(r) => Some(r),
            TypeView::TemplateSpecialization { name, .. } => self
                .ast
                .types
                .template_decl(name)
                .and_then(|t| self.ast.template_info(t))
                .and_then(|t| t.pattern),
            _ => self.ast.types.as_record(c),
        };
        let Some(r) = record else { return };
        if !is_record(self.kind_of(r)) {
            return;
        }
        let has_attr = self.ast.redecls_of(r).any(|x| {
            self.ast.lists.iter::<AttrId>(self.ast.attrs.of_decl(x)).any(|a| {
                matches!(self.ast.attrs.get(a).kind, AttrKind::GSL_OWNER | AttrKind::GSL_POINTER)
            })
        });
        if !has_attr {
            let first = self.ast.first(r);
            self.gsl_inferred_pointers.insert(first);
        }
    }

    /// The typedef instantiated from pattern `pattern` as `td`: clang
    /// infers again only when the pattern names a dependent name
    /// (`typename T::iterator`).
    pub(crate) fn infer_gsl_instantiated_typedef(&mut self, pattern: DeclId, td: DeclId) {
        let Some(mut u) = self.ast.decls.typedef(pattern).map(|x| x.underlying) else { return };
        for _ in 0..64 {
            match self.ast.types.view(u.ty()) {
                TypeView::DependentName { .. } => {
                    self.infer_gsl_typedef(td);
                    return;
                }
                _ => {
                    let next = self.ast.types.desugar_once(u);
                    if next == u {
                        return;
                    }
                    u = next;
                }
            }
        }
    }
}
