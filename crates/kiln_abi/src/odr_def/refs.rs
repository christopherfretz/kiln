//! Names and types in the definition hash (O-10 "Referenced entities by
//! name", "Types by mangled encoding"; mortar's ruling 5): an external
//! entity by its mangled name, a type by its `<type>` encoding, each
//! memoized (per first declaration, per canonical type) so a unit pays
//! one mangling per entity and type; and the guard against components
//! numbered per unit, decided on the AST.

use kiln_ast::decl::DeclKind;
use kiln_ast::{Ast, DeclId, QualType};
use kiln_source::fx::FxHashMap;

use super::DefStats;
use crate::mangle::{self, AnonIds};
use crate::odr::{NotTuLocal, fnv1a_128};

/// The memo of names and types (`None`: the per-TU placeholder).
#[derive(Debug)]
pub(super) struct Refs<'a> {
    ast: &'a Ast,
    /// The hasher's own mangling state (never the module's).
    anon: AnonIds,
    test: NotTuLocal<'a>,
    entities: FxHashMap<DeclId, Option<u128>>,
    tags: FxHashMap<DeclId, Option<u128>>,
    types: FxHashMap<QualType, Option<u128>>,
    /// Every string hashed, when a test asks.
    pub(super) names: Option<Vec<String>>,
}

impl<'a> Refs<'a> {
    pub(super) fn new(ast: &'a Ast, anon: AnonIds) -> Self {
        Refs {
            ast,
            anon,
            test: NotTuLocal::new(ast),
            entities: FxHashMap::default(),
            tags: FxHashMap::default(),
            types: FxHashMap::default(),
            names: None,
        }
    }

    /// The heap of the memo maps, in bytes.
    pub(super) fn memory(&self) -> usize {
        let entry = size_of::<(DeclId, Option<u128>)>() + 1;
        (self.entities.capacity() + self.tags.capacity()) * entry
            + self.types.capacity() * (size_of::<(QualType, Option<u128>)>() + 1)
    }

    fn digest(&mut self, s: String) -> u128 {
        let h = fnv1a_128(&s);
        if let Some(n) = &mut self.names {
            n.push(s);
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
        if let Some(&r) = self.types.get(&c) {
            st.type_hits += 1;
            return r;
        }
        st.type_misses += 1;
        let r = if self.test.member_ty(c) {
            let s = mangle::mangle_type(self.ast, &mut self.anon, c);
            Some(self.digest(s))
        } else {
            None
        };
        self.types.insert(c, r);
        r
    }

    /// Class or enumeration `d` by its `<type>` encoding (what `_ZTS`
    /// names), or `None` when it fails O-17's test.
    pub(super) fn tag(&mut self, d: DeclId, st: &mut DefStats) -> Option<u128> {
        let first = self.ast.first(d);
        if let Some(&r) = self.tags.get(&first) {
            st.type_hits += 1;
            return r;
        }
        st.type_misses += 1;
        let r = if self.test.tag(first) {
            let s = mangle::mangle_class_special(self.ast, &mut self.anon, "", first);
            Some(self.digest(s))
        } else {
            None
        };
        self.tags.insert(first, r);
        r
    }

    /// Function or variable `d` by its mangled name, as kiln would emit
    /// the symbol; `None` when the name may hold a per-TU component.
    pub(super) fn entity(&mut self, d: DeclId, st: &mut DefStats) -> Option<u128> {
        let first = self.ast.first(d);
        if let Some(&r) = self.entities.get(&first) {
            st.entity_hits += 1;
            return r;
        }
        st.entity_misses += 1;
        let r = if self.entity_passes(first) {
            let s = mangle::mangle_decl(self.ast, &mut self.anon, first);
            // A function template specialization's encoding is the
            // template's signature, whose dependent parts the AST of the
            // specialization does not show: read it.
            if self.is_function_spec(first) && has_per_tu_component(&s) {
                None
            } else {
                Some(self.digest(s))
            }
        } else {
            None
        };
        self.entities.insert(first, r);
        r
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
