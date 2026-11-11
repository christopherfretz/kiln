//! Flexible array members in C++ (spec/sema.md §5.8; a C99 feature and a
//! GNU extension in C++, clang `Sema::ActOnFields`).
//!
//! A non-static data member of array-of-unknown-bound type is accepted as
//! the last member of a class (or as any member of a union). The member
//! keeps its incomplete type `T[]`, the class is complete, and the layout
//! gives it size zero at the aligned end of the preceding members
//! (`kiln_abi::layout`).

use kiln_ast::decl::{DeclFlags, DeclKind, DefinitionBits, TagKind};
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, QualType};
use kiln_diag::DiagId;

use crate::init::{InitItem, InitValue};

use crate::Sema;

impl Sema<'_, '_> {
    /// True if `q` is an array of unknown bound (the type of a flexible
    /// array member).
    pub(crate) fn is_flexible_array_type(&self, q: QualType) -> bool {
        matches!(self.ast.types.canonical_view(q), TypeView::IncompleteArray(_))
    }

    /// True if the class type `q` has a flexible array member
    /// ([`DefinitionBits::HAS_FLEXIBLE_ARRAY_MEMBER`]).
    pub(crate) fn has_flexible_array_member(&self, q: QualType) -> bool {
        self.ast
            .types
            .as_record(q)
            .is_some_and(|r| self.defn_bits(r).contains(DefinitionBits::HAS_FLEXIBLE_ARRAY_MEMBER))
    }

    /// The flexible-array-member checks of class `r` at its `}` (clang
    /// `ActOnFields`), over its non-static data members in order. Returns
    /// whether the class has a flexible array member.
    ///
    /// - Not the last member of a struct/class: `flexible array member 'x'
    ///   with type 'T[]' is not at the end of struct` plus `next field
    ///   declaration is here`; the member and the class are invalid.
    /// - In a union: `-Wgnu-flexible-array-union-member`; the only named
    ///   member: `-Wgnu-empty-struct` (both Extensions).
    /// - In a class with a virtual base: an error.
    /// - Always (C++ is not C99): `flexible array members are a C99
    ///   feature` [-Wc99-extensions] (an Extension).
    /// - An element type with a non-trivial destructor: an error, and the
    ///   member and the class are invalid.
    /// - A member of a class type that has a flexible array member makes
    ///   this class have one too; in a struct/class it warns
    ///   `-Wgnu-variable-sized-type-not-at-end` when not last, else
    ///   `-Wflexible-array-extensions` (an Extension).
    pub(crate) fn check_flexible_array_members(&mut self, r: DeclId) -> bool {
        let fields: Vec<DeclId> = self
            .ast
            .decls
            .members(r)
            .filter(|&m| self.ast.decls.get(m).kind() == DeclKind::Field)
            .collect();
        let tag = self.ast.decls.record(r).map_or(TagKind::Struct, |x| x.tag);
        let union = tag == TagKind::Union;
        let mut named = 0usize;
        let mut has = false;
        for (i, &f) in fields.iter().enumerate() {
            let n = *self.ast.decls.get(f);
            let last = i + 1 == fields.len();
            let Some(ty) = n.ty().filter(|_| !n.has(DeclFlags::INVALID)) else {
                if !n.name().is_empty() {
                    named += 1;
                }
                continue;
            };
            let name = self.decl_name(f);
            if self.is_flexible_array_type(ty) {
                if !union && !last {
                    let next = self.ast.decls.get(fields[i + 1]).loc();
                    let note = self.diag(DiagId::note_next_field_declaration, next);
                    let msg = self
                        .diag(DiagId::sema_flexible_array_not_at_end, n.loc())
                        .arg(&name)
                        .arg(self.ty_arg(ty))
                        .arg(tag as u8);
                    self.emit(msg.with_note(note).per_instantiation());
                    self.ast.decls.set_flags(f, DeclFlags::INVALID);
                    self.ast.decls.set_flags(r, DeclFlags::INVALID);
                    continue;
                }
                if union {
                    let msg = self.diag(DiagId::sema_flexible_array_union, n.loc()).arg(&name);
                    self.emit(msg.per_instantiation());
                } else if named == 0 {
                    let msg = self
                        .diag(DiagId::sema_flexible_array_empty_aggregate, n.loc())
                        .arg(&name)
                        .arg(tag as u8);
                    self.emit(msg.per_instantiation());
                }
                if self.class_has_virtual_bases(r) {
                    let msg = self
                        .diag(DiagId::sema_flexible_array_virtual_base, n.loc())
                        .arg(&name)
                        .arg(tag as u8);
                    self.emit(msg.per_instantiation());
                }
                let msg = self.diag(DiagId::sema_c99_flexible_array_member, n.loc());
                self.emit(msg.per_instantiation());
                let elem = self.base_element(ty);
                if !self.is_dependent_type(elem) && self.is_destructed_type(elem) {
                    let msg = self
                        .diag(DiagId::sema_flexible_array_nontrivial_dtor, n.loc())
                        .arg(&name)
                        .arg(self.ty_arg(ty));
                    self.emit(msg.per_instantiation());
                    self.ast.decls.set_flags(f, DeclFlags::INVALID);
                    self.ast.decls.set_flags(r, DeclFlags::INVALID);
                    continue;
                }
                has = true;
            } else if self.has_flexible_array_member(ty) {
                has = true;
                if !union {
                    let msg = if last {
                        self.diag(DiagId::sema_flexible_array_in_struct, n.loc()).arg(&name)
                    } else {
                        self.diag(DiagId::sema_variable_sized_type_in_struct, n.loc())
                            .arg(&name)
                            .arg(self.ty_arg(ty))
                    };
                    self.emit(msg.per_instantiation());
                }
            }
            if !n.name().is_empty() {
                named += 1;
            }
        }
        has
    }

    /// True if class `r` (being completed) has a virtual base, direct or
    /// indirect (clang `getNumVBases() != 0`).
    fn class_has_virtual_bases(&self, r: DeclId) -> bool {
        self.ast.decls.bases(r, &self.ast.lists).any(|b| {
            b.is_virtual
                || self
                    .ast
                    .types
                    .as_record(b.ty)
                    .is_some_and(|d| self.defn_bits(d).contains(DefinitionBits::HAS_VIRTUAL_BASES))
        })
    }
}

impl Sema<'_, '_> {
    /// The initializer `item` of flexible array member `f` in an aggregate
    /// initialization (clang `CheckFlexibleArrayInit`). An empty braced
    /// list is a GNU extension (`-Wgnu-flexible-array-initializer`) and
    /// leaves the member without elements, as does a member with no
    /// initializer. A non-empty one is an error unless it initializes a
    /// variable with static or thread storage duration directly (not a
    /// nested subobject), where it is the same extension: the member is
    /// then initialized as an array of unknown bound, its elements laid
    /// out past the object (WP-1286). `None` on an error, else whether
    /// the member has elements to initialize.
    pub(crate) fn check_flexible_array_init(&mut self, f: DeclId, item: &InitItem) -> Option<bool> {
        let floc = self.ast.decls.get(f).loc();
        let note = self.diag(DiagId::note_flexible_array_member, floc).arg(self.decl_name(f));
        let (at, empty) = match &item.value {
            InitValue::List(l) => (l.lbrace, l.items.is_empty()),
            InitValue::Expr(e) => (self.begin_loc(*e), false),
        };
        let top_static =
            self.init_entity.is_none() && self.init_var.is_some_and(|v| !self.has_local_storage(v));
        if empty || top_static {
            let msg = self.diag(DiagId::sema_flexible_array_init_ext, at).with_note(note);
            self.emit(msg);
            if !empty {
                return Some(true);
            }
            // clang `CheckArrayType`: the member is then a zero-length
            // array.
            self.error(DiagId::sema_zero_length_array, at);
            return Some(false);
        }
        let msg = self.diag(DiagId::sema_flexible_array_init, at).with_note(note);
        self.emit(msg);
        None
    }
}
