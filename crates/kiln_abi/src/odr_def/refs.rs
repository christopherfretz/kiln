//! Names and types in the definition hash (O-10 "Referenced entities by
//! name", "Types by mangled encoding"; mortar's ruling 5): an external
//! entity by its mangled name, a type by its `<type>` encoding, each
//! memoized (per first declaration, per canonical type) so a unit pays
//! one mangling per entity and type; and the guard against components
//! numbered per unit, decided on the AST.
//!
//! Statistics only (WP-1627): each memo entry also says whether the item
//! names an entity or type declared in a system header, and every item
//! the memo hands out ORs that into [`Refs::named_system`]. No byte of
//! the stream depends on it.

use kiln_ast::decl::DeclKind;
use kiln_ast::types::{ArgView, TemplateName, TypeView};
use kiln_ast::{Ast, DeclId, QualType};
use kiln_source::SourceManager;
use kiln_source::fx::FxHashMap;

use super::{DefStats, KnownNames};
use crate::mangle::{self, AnonIds};
use crate::odr::{NotTuLocal, fnv1a_128};

/// The memo of names and types (`None`: the per-TU placeholder).
#[derive(Debug)]
pub(super) struct Refs<'a> {
    ast: &'a Ast,
    sm: &'a SourceManager,
    /// The hasher's own mangling state (never the module's).
    anon: AnonIds,
    test: NotTuLocal<'a>,
    /// Per first declaration (entities) and canonical type: the digest
    /// (`None`: the placeholder) and whether it names a system entity.
    entities: FxHashMap<DeclId, (Option<u128>, bool)>,
    tags: FxHashMap<DeclId, (Option<u128>, bool)>,
    types: FxHashMap<QualType, (Option<u128>, bool)>,
    /// Per first declaration of a class, enumeration or template: whether
    /// it or what names it (template arguments, an enclosing class) is
    /// declared in a system header (WP-1627).
    system: FxHashMap<DeclId, bool>,
    /// Set by every item handed out that names a system entity or type
    /// (WP-1627 statistics; the hasher saves and restores it around its
    /// own memoized sub-streams).
    pub(super) named_system: bool,
    /// Every string hashed, when a test asks.
    pub(super) names: Option<Vec<String>>,
    /// The names codegen already produced.
    pub(super) known: Option<&'a dyn KnownNames>,
    /// Whether the module has an `abi_tag` attribute: codegen's names
    /// may then carry tags, which the hash leaves out, so they are not
    /// reused.
    tagged: bool,
}

impl std::fmt::Debug for dyn KnownNames + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KnownNames")
    }
}

impl<'a> Refs<'a> {
    /// `anon` is mangled without abi tags (an entity's tags, own or
    /// inherited, are not part of its identity in the hash; O-10).
    pub(super) fn new(ast: &'a Ast, sm: &'a SourceManager, anon: AnonIds) -> Self {
        Refs {
            ast,
            sm,
            anon: anon.without_abi_tags(),
            tagged: ast.attrs.has_kind(kiln_ast::attr::AttrKind::ABI_TAG),
            test: NotTuLocal::new(ast),
            entities: FxHashMap::default(),
            tags: FxHashMap::default(),
            types: FxHashMap::default(),
            system: FxHashMap::default(),
            named_system: false,
            names: None,
            known: None,
        }
    }

    /// The hasher's own mangling state.
    pub(super) fn anon_mut(&mut self) -> &mut AnonIds {
        &mut self.anon
    }

    /// Whether `d` (a function or variable) passes the per-TU guard on
    /// its scopes, template arguments and type (the guard of
    /// [`Self::entity`], without mangling).
    pub(super) fn passes(&mut self, d: DeclId) -> bool {
        let first = self.ast.first(d);
        self.entity_passes(first)
    }

    /// The heap of the memo maps, in bytes.
    pub(super) fn memory(&self) -> usize {
        let entry = size_of::<(DeclId, (Option<u128>, bool))>() + 1;
        (self.entities.capacity() + self.tags.capacity()) * entry
            + self.types.capacity() * (size_of::<(QualType, (Option<u128>, bool))>() + 1)
            + self.system.capacity() * (size_of::<(DeclId, bool)>() + 1)
    }

    fn digest(&mut self, s: &str) -> u128 {
        let h = fnv1a_128(s);
        if let Some(n) = &mut self.names {
            n.push(s.to_owned());
        }
        h
    }

    /// The canonical type `q` by its `<type>` encoding; `None` when the
    /// type is not built only from fundamental types and classes and
    /// enumerations that pass O-17's syntactic test ([`NotTuLocal`]):
    /// such an encoding may hold an unnamed-namespace, local-entity,
    /// unnamed-type or closure component.
    pub(super) fn ty(&mut self, q: QualType, st: &mut DefStats) -> Option<u128> {
        let c = self.ast.types.canonical(q);
        if let Some(&(r, sys)) = self.types.get(&c) {
            st.type_hits += 1;
            self.named_system |= sys && r.is_some();
            return r;
        }
        st.type_misses += 1;
        let r = if self.test.member_ty(c) {
            let s = mangle::mangle_type(self.ast, &mut self.anon, c);
            Some(self.digest(&s))
        } else {
            None
        };
        let sys = r.is_some() && self.type_system(c, 0);
        self.types.insert(c, (r, sys));
        self.named_system |= sys;
        r
    }

    /// Class or enumeration `d` by its `<type>` encoding (what `_ZTS`
    /// names), or `None` when it fails O-17's test.
    pub(super) fn tag(&mut self, d: DeclId, st: &mut DefStats) -> Option<u128> {
        let first = self.ast.first(d);
        if let Some(&(r, sys)) = self.tags.get(&first) {
            st.type_hits += 1;
            self.named_system |= sys;
            return r;
        }
        st.type_misses += 1;
        let r = if self.test.tag(first) {
            let s = mangle::mangle_class_special(self.ast, &mut self.anon, "", first);
            Some(self.digest(&s))
        } else {
            None
        };
        let sys = r.is_some() && self.tag_system(first, 0);
        self.tags.insert(first, (r, sys));
        self.named_system |= sys;
        r
    }

    /// Function or variable `d` by its mangled name, as kiln would emit
    /// the symbol; `None` when the name may hold a per-TU component.
    pub(super) fn entity(&mut self, d: DeclId, st: &mut DefStats) -> Option<u128> {
        let first = self.ast.first(d);
        if let Some(&(r, sys)) = self.entities.get(&first) {
            st.entity_hits += 1;
            self.named_system |= sys;
            return r;
        }
        st.entity_misses += 1;
        let r = if self.entity_passes(first) {
            // The name codegen gave the symbol, else a mangling of our own
            // (the guard above applies to both).
            let known = self.known.filter(|_| !self.tagged).and_then(|k| k.name(first));
            st.entity_known += u64::from(known.is_some());
            let owned;
            let s = if let Some(s) = known {
                s
            } else {
                owned = mangle::mangle_decl(self.ast, &mut self.anon, first);
                owned.as_str()
            };
            // A function template specialization's encoding is the
            // template's signature, whose dependent parts the AST of the
            // specialization does not show: read it.
            if self.is_function_spec(first) && has_per_tu_component(s) {
                None
            } else {
                let h = fnv1a_128(s);
                if let Some(n) = &mut self.names {
                    n.push(s.to_owned());
                }
                Some(h)
            }
        } else {
            None
        };
        let sys = r.is_some() && self.entity_system(first);
        self.entities.insert(first, (r, sys));
        self.named_system |= sys;
        r
    }

    /// Whether the first declaration `first` lies in a system header
    /// (`#include <...>` from a system directory, `-isystem`, `#pragma GCC
    /// system_header`; [`SourceManager::is_in_system_header`]).
    fn loc_system(&self, first: DeclId) -> bool {
        self.sm.is_in_system_header(self.ast.decls.get(first).loc())
    }

    /// Function or variable `first` names a system entity: it is declared
    /// in a system header, a member of a class that is or names one, or a
    /// specialization with such an argument (WP-1627).
    pub(super) fn entity_system(&mut self, first: DeclId) -> bool {
        if self.loc_system(first) {
            return true;
        }
        let ast = self.ast;
        if let Some((_, s)) = ast.spec_info(first)
            && self.args_system(s.args, 0)
        {
            return true;
        }
        ast.decls.get(first).ctx().is_some_and(|c| {
            matches!(
                ast.decls.get(c).kind(),
                DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization
            ) && self.tag_system(ast.first(c), 0)
        })
    }

    /// Template `tmpl` (first declaration) and template argument list
    /// `args`: whether either names a system entity (a function template
    /// specialization item, which takes neither through the memo).
    pub(super) fn template_item_system(
        &mut self,
        tmpl: DeclId,
        args: kiln_ast::ids::ArgListId,
    ) -> bool {
        self.loc_system(tmpl) || self.args_system(args, 0)
    }

    /// Class or enumeration `first` (first declaration): declared in a
    /// system header, in a class that is or names one, or a
    /// specialization with such an argument. Memoized.
    fn tag_system(&mut self, first: DeclId, depth: u32) -> bool {
        if let Some(&r) = self.system.get(&first) {
            return r;
        }
        if depth > 32 {
            return false;
        }
        self.system.insert(first, false);
        let ast = self.ast;
        let r = self.loc_system(first)
            || ast.spec_info(first).is_some_and(|(_, s)| self.args_system(s.args, depth + 1))
            || ast.decls.get(first).ctx().is_some_and(|c| {
                matches!(
                    ast.decls.get(c).kind(),
                    DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization
                ) && self.tag_system(ast.first(c), depth + 1)
            });
        self.system.insert(first, r);
        r
    }

    /// Whether canonical type `c` holds a class or enumeration that
    /// names a system entity (through pointers, references, arrays,
    /// function and member pointer types).
    fn type_system(&mut self, c: QualType, depth: u32) -> bool {
        let ast = self.ast;
        if depth > 32 {
            return false;
        }
        let c = ast.types.canonical(c);
        match ast.types.canonical_view(c) {
            TypeView::Record(d) | TypeView::Enum(d) => self.tag_system(ast.first(d), depth + 1),
            TypeView::Pointer(p)
            | TypeView::LValueRef(p)
            | TypeView::RValueRef(p)
            | TypeView::Complex(p)
            | TypeView::Atomic(p) => self.type_system(p, depth + 1),
            TypeView::ConstantArray(_) | TypeView::IncompleteArray(_) => {
                ast.types.element(c).is_some_and(|e| self.type_system(e, depth + 1))
            }
            TypeView::Vector { elem, .. } => self.type_system(elem, depth + 1),
            TypeView::MemberPointer { pointee, class } => {
                self.type_system(pointee, depth + 1)
                    || self.type_system(QualType::unqual(class), depth + 1)
            }
            TypeView::Function(p) => {
                self.type_system(p.ret(), depth + 1)
                    || ast.types.params(&p).iter().any(|&x| self.type_system(x, depth + 1))
            }
            _ => false,
        }
    }

    /// Whether template argument list `list` names a system entity.
    fn args_system(&mut self, list: kiln_ast::ids::ArgListId, depth: u32) -> bool {
        let ast = self.ast;
        let list = ast.types.canonical_args(list);
        ast.types.args(list).iter().any(|a| match a.view() {
            ArgView::Type(q) | ArgView::Value { ty: q, .. } => self.type_system(q, depth + 1),
            ArgView::Template(t) | ArgView::TemplateExpansion { name: t, .. } => {
                match ast.types.template_name_view(ast.types.canonical_template_name(t)) {
                    TemplateName::Template(d) => self.loc_system(ast.first(d)),
                    _ => false,
                }
            }
            ArgView::Pack(l) => self.args_system(l, depth + 1),
            _ => false,
        })
    }

    fn is_function_spec(&self, d: DeclId) -> bool {
        self.ast.decls.function(d).is_some() && self.ast.spec_info(d).is_some()
    }

    /// The structural guard: every enclosing scope a named namespace or a
    /// passing class (no unnamed namespace, no enclosing function or
    /// closure), template arguments that pass, and a declared type that
    /// passes (a function's parameter and return types are part of its
    /// encoding).
    fn entity_passes(&mut self, d: DeclId) -> bool {
        let ast = self.ast;
        let n = ast.decls.get(d);
        if !matches!(
            n.kind(),
            DeclKind::Function
                | DeclKind::CXXMethod
                | DeclKind::CXXConstructor
                | DeclKind::CXXDestructor
                | DeclKind::CXXConversion
                | DeclKind::Var
                | DeclKind::VarTemplateSpecialization
        ) || n.name().is_empty()
        {
            return false;
        }
        if !self.test.scopes(d) {
            return false;
        }
        if let Some((_, s)) = ast.spec_info(d) {
            let args = ast.types.canonical_args(s.args);
            if !self.test.args(args) {
                return false;
            }
        }
        n.ty().is_some_and(|t| self.test.member_ty(t))
    }
}

/// Whether mangled name `enc` holds a component numbered per unit
/// (`_GLOBAL__N`, a local name `Z...E`, an unnamed type `Ut`/`Ul`, an
/// unnumbered `$_N`). Source names are skipped by their length, literals
/// `L ... E` whole, and `L_Z <encoding> E` read as an external name.
pub(crate) fn has_per_tu_component(enc: &str) -> bool {
    let b = enc.as_bytes();
    // The leading `_Z` of the name itself.
    let mut i = if enc.starts_with("_Z") { 2 } else { 0 };
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_digit() {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            let n: usize = enc[start..i].parse().unwrap_or(usize::MAX);
            let name = &b[i..i.saturating_add(n).min(b.len())];
            if name.starts_with(b"_GLOBAL__N") || name.starts_with(b"$") {
                return true;
            }
            i = i.saturating_add(n);
            continue;
        }
        match c {
            b'Z' => return true,
            b'U' if matches!(b.get(i + 1), Some(b't' | b'l')) => return true,
            b'L' if b.get(i + 1) == Some(&b'_') && b.get(i + 2) == Some(&b'Z') => i += 2,
            b'L' if b.get(i + 1).is_some_and(u8::is_ascii_lowercase) => {
                while i < b.len() && b[i] != b'E' {
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    false
}
