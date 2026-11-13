//! The schema hash (spec/snap.md "Schema hash").
//!
//! Two forms of one description. [`Snap::SCHEMA_HASH`] is a constant,
//! folded at compile time by [`SchemaHash`] from the type's own
//! description and its encoded field types' constants: what a snapshot
//! stores and a consumer compares, at no cost at run time (WP-1401). The
//! descriptor walk below is the readable form, for tests and for finding
//! which type changed.
//!
//! Every [`Snap`] type describes itself once in a [`Schema`]: its
//! `std::any::type_name`, its kind, its fields (name and field type name,
//! in order) or variants (name and explicit discriminant), and the
//! attributes that change its encoding. [`schema_hash`] hashes the sorted
//! descriptors of everything reachable from a root. A snapshot is only
//! read by a build whose hash matches the writer's, so a struct-layout
//! change anywhere below the root is a clean refusal, not a misread.

use std::any::{TypeId, type_name};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fmt::Write as _;

use crate::Snap;

/// The descriptor of one type: built with [`Desc::new`] and the field and
/// variant methods, in declaration order, then passed to
/// [`Schema::register`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Desc(String, Option<u64>, Vec<(&'static str, bool)>);

impl Desc {
    /// A descriptor of kind `kind` (`"struct"`, `"enum"`, `"u32"`,
    /// `"vec"`, `"root"`, ...).
    pub fn new(kind: &str) -> Self {
        Desc(kind.to_owned(), None, Vec::new())
    }

    /// The type's [`Snap::SCHEMA_HASH`] (or a root's constant), recorded
    /// beside the descriptor for [`Schema::hashes`]; not part of the
    /// descriptor's text.
    #[must_use]
    pub fn hash(mut self, h: u64) -> Self {
        self.1 = Some(h);
        self
    }

    /// An encoded field `name` of type `T`.
    #[must_use]
    pub fn field<T: ?Sized>(mut self, name: &str) -> Self {
        let _ = write!(self.0, " {name}:{}", type_name::<T>());
        self.2.push((type_name::<T>(), false));
        self
    }

    /// An encoded field `name` of type `T` that closes a cycle of types
    /// ([`SchemaHash::cycle`]): the same descriptor text as
    /// [`Desc::field`]; [`Schema::unfolded`] counts only `T`'s own items
    /// as folded through it.
    #[must_use]
    pub fn cycle<T: ?Sized>(mut self, name: &str) -> Self {
        let _ = write!(self.0, " {name}:{}", type_name::<T>());
        self.2.push((type_name::<T>(), true));
        self
    }

    /// A field `name` of type `T` that is not encoded (`#[snap(skip)]`,
    /// or rebuilt by a hand-written root).
    #[must_use]
    pub fn skip<T: ?Sized>(mut self, name: &str) -> Self {
        let _ = write!(self.0, " {name}:skip:{}", type_name::<T>());
        self
    }

    /// A field `name` of type `T` encoded by the module `path`
    /// (`#[snap(with = "path")]`).
    #[must_use]
    pub fn with<T: ?Sized>(mut self, name: &str, path: &str) -> Self {
        let _ = write!(self.0, " {name}:with({path}):{}", type_name::<T>());
        self
    }

    /// An enum variant `name` with explicit discriminant `disc` (`""` if
    /// none); its fields follow.
    #[must_use]
    pub fn variant(mut self, name: &str, disc: &str) -> Self {
        let _ = write!(self.0, " | {name}");
        if !disc.is_empty() {
            let _ = write!(self.0, "={disc}");
        }
        self
    }

    /// Free text that changes the encoding of a hand-written impl (a
    /// format revision, a parameter): bump it when the encoding changes
    /// without a field change.
    #[must_use]
    pub fn note(mut self, text: &str) -> Self {
        let _ = write!(self.0, " #{text}");
        self
    }
}

/// The set of type descriptors reachable from a root.
pub struct Schema {
    seen: HashSet<TypeId>,
    descs: Vec<String>,
    hashes: Vec<Option<u64>>,
    names: Vec<&'static str>,
    /// Each type's encoded field types, and whether the field closes a
    /// cycle ([`Desc::cycle`]).
    edges: Vec<Vec<(&'static str, bool)>>,
}

impl Default for Schema {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Schema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Schema").field("types", &self.descs.len()).finish_non_exhaustive()
    }
}

impl Schema {
    /// An empty schema.
    pub fn new() -> Self {
        Schema {
            seen: HashSet::new(),
            descs: Vec::new(),
            hashes: Vec::new(),
            names: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Registers `T` with descriptor `desc`. Returns `true` the first time
    /// for `T`: the caller then calls `schema` on each encoded field type.
    /// Registering before recursing makes recursive types terminate.
    pub fn register<T: ?Sized + 'static>(&mut self, desc: Desc) -> bool {
        if !self.seen.insert(TypeId::of::<T>()) {
            return false;
        }
        let Desc(text, hash, edges) = desc;
        self.descs.push(format!("{} = {text}", type_name::<T>()));
        self.hashes.push(hash);
        self.names.push(type_name::<T>());
        self.edges.push(edges);
        true
    }

    /// The types whose own items the constant of a root does not fold:
    /// `(root, type)` for each type a root (a type no registered type
    /// encodes) reaches but whose items its [`Snap::SCHEMA_HASH`] misses.
    /// A constant folds what an encoded field reaches, except through a
    /// field that closes a cycle ([`Desc::cycle`]), which folds only the
    /// field type's own items ([`Snap::SHAPE_HASH`]). Empty when the
    /// cycle-closing fields are placed so that every reached type is
    /// still folded on another path: then any change of any reached type
    /// changes the root's constant.
    pub fn unfolded(&self) -> Vec<(&'static str, &'static str)> {
        let at: HashMap<&str, usize> =
            self.names.iter().enumerate().map(|(i, n)| (*n, i)).collect();
        let n = self.names.len();
        let kids = |i: usize| self.edges[i].iter().filter_map(|&(t, c)| Some((*at.get(t)?, c)));
        let mut encoded = vec![false; n];
        for i in 0..n {
            for (j, _) in kids(i) {
                encoded[j] |= j != i;
            }
        }
        let mut out = Vec::new();
        for root in (0..n).filter(|&i| !encoded[i]) {
            let (mut reached, mut folded, mut expanded) =
                (vec![false; n], vec![false; n], vec![false; n]);
            let mut todo = vec![root];
            reached[root] = true;
            while let Some(i) = todo.pop() {
                for (j, _) in kids(i) {
                    if !reached[j] {
                        reached[j] = true;
                        todo.push(j);
                    }
                }
            }
            let mut todo = vec![root];
            folded[root] = true;
            while let Some(i) = todo.pop() {
                if std::mem::replace(&mut expanded[i], true) {
                    continue;
                }
                for (j, cycle) in kids(i) {
                    folded[j] = true;
                    if !cycle {
                        todo.push(j);
                    }
                }
            }
            out.extend(
                (0..n)
                    .filter(|&i| reached[i] && !folded[i])
                    .map(|i| (self.names[root], self.names[i])),
            );
        }
        out
    }

    /// Each descriptor with the compile-time hash its type declared
    /// ([`Desc::hash`]), in registration order: the cross-check of the
    /// two forms (two types with different descriptors must not declare
    /// the same constant, and every type declares one).
    pub fn hashes(&self) -> Vec<(&str, Option<u64>)> {
        self.descs.iter().map(String::as_str).zip(self.hashes.iter().copied()).collect()
    }

    /// The descriptors, sorted (what [`Schema::hash`] hashes).
    pub fn descriptors(&self) -> Vec<&str> {
        let mut d: Vec<&str> = self.descs.iter().map(String::as_str).collect();
        d.sort_unstable();
        d
    }

    /// A 64-bit hash of the sorted descriptors (FNV-1a over each
    /// descriptor and a separator): equal across builds of the same
    /// sources, since descriptors hold only names.
    pub fn hash(&self) -> u64 {
        let mut h = FNV_OFFSET;
        for d in self.descriptors() {
            for &b in d.as_bytes().iter().chain(&[0xff]) {
                h ^= u64::from(b);
                h = h.wrapping_mul(FNV_PRIME);
            }
        }
        h
    }
}

/// The FNV-1a 64 offset basis and prime of [`SchemaHash`] and
/// [`Schema::hash`].
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// The compile-time schema hash of one type (spec/snap.md "Schema
/// hash"): FNV-1a 64 folded over the type's description, item by item,
/// in `const fn`s, so that [`Snap::SCHEMA_HASH`] is a constant. Each item
/// is its bytes and a separator byte of its own kind, so no two item
/// sequences fold the same bytes; an encoded field contributes its type's
/// own constant ([`SchemaHash::field`]), so a change anywhere below a
/// type changes the type's constant. The derive writes these calls from
/// the same tokens as the descriptor; a hand-written impl writes them
/// beside its [`Desc`], item for item.
///
/// ```
/// use kiln_snap::{SchemaHash, Snap};
/// const A: u64 = SchemaHash::new("struct", "S").field::<u32>("a").finish();
/// const B: u64 = SchemaHash::new("struct", "S").field::<u64>("a").finish();
/// assert_ne!(A, B);
/// assert_eq!(<u32 as Snap>::SCHEMA_HASH, <u32 as Snap>::SCHEMA_HASH);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchemaHash(u64);

impl SchemaHash {
    /// A type of kind `kind` (as [`Desc::new`]) named `name` (the type's
    /// name as written, without its module: two copies of a type in two
    /// modules encode alike, so they may hash alike).
    #[must_use]
    pub const fn new(kind: &str, name: &str) -> Self {
        SchemaHash(FNV_OFFSET).bytes(kind.as_bytes(), 0xf1).bytes(name.as_bytes(), 0xf2)
    }

    const fn bytes(self, b: &[u8], sep: u8) -> Self {
        let mut h = self.0;
        let mut i = 0;
        while i < b.len() {
            h = (h ^ b[i] as u64).wrapping_mul(FNV_PRIME);
            i += 1;
        }
        SchemaHash((h ^ sep as u64).wrapping_mul(FNV_PRIME))
    }

    /// Folds another type's (or a root's) constant `h`.
    #[must_use]
    pub const fn word(self, h: u64) -> Self {
        self.bytes(&h.to_le_bytes(), 0xf3)
    }

    /// An encoded field `name` of a type that is not [`Snap`] itself (a
    /// configured root such as the Oven), whose constant is `h`.
    #[must_use]
    pub const fn root_field(self, name: &str, h: u64) -> Self {
        self.bytes(name.as_bytes(), 0xf4).word(h)
    }

    /// An encoded field `name` of type `T` (as [`Desc::field`]).
    #[must_use]
    pub const fn field<T: Snap>(self, name: &str) -> Self {
        self.bytes(name.as_bytes(), 0xf4).word(T::SCHEMA_HASH)
    }

    /// A field `name` that is not encoded, of the type spelled `ty` (as
    /// [`Desc::skip`]).
    #[must_use]
    pub const fn skip(self, name: &str, ty: &str) -> Self {
        self.bytes(name.as_bytes(), 0xf5).bytes(ty.as_bytes(), 0xf6)
    }

    /// A field `name` of the type spelled `ty` encoded by the module
    /// `path` (as [`Desc::with`]).
    #[must_use]
    pub const fn with(self, name: &str, path: &str, ty: &str) -> Self {
        self.bytes(name.as_bytes(), 0xf7).bytes(path.as_bytes(), 0xf8).bytes(ty.as_bytes(), 0xf6)
    }

    /// An encoded field `name` of the type spelled `ty`, by its spelling
    /// alone: a derived type's [`Snap::SHAPE_HASH`] folds its fields so,
    /// which depends on no other constant.
    #[must_use]
    pub const fn spelled(self, name: &str, ty: &str) -> Self {
        self.bytes(name.as_bytes(), 0xf9).bytes(ty.as_bytes(), 0xf6)
    }

    /// An encoded field `name` of type `T` that closes a cycle of types
    /// (`T` is, or reaches, the type described: `T::SCHEMA_HASH` would
    /// depend on itself): folds `T`'s own items, [`Snap::SHAPE_HASH`]
    /// (as [`Desc::cycle`]). The derive uses it for a field whose type
    /// names the type itself and for a field marked `#[snap(cycle)]`.
    #[must_use]
    pub const fn cycle<T: Snap>(self, name: &str) -> Self {
        self.bytes(name.as_bytes(), 0xfe).word(T::SHAPE_HASH)
    }

    /// An element `name` of type `T` in a container's
    /// [`Snap::SHAPE_HASH`]: the element's own `SHAPE_HASH`.
    #[must_use]
    pub const fn shape<T: Snap>(self, name: &str) -> Self {
        self.bytes(name.as_bytes(), 0xf4).bytes(&T::SHAPE_HASH.to_le_bytes(), 0xff)
    }

    /// An enum variant `name` with explicit discriminant `disc` (as
    /// [`Desc::variant`]).
    #[must_use]
    pub const fn variant(self, name: &str, disc: &str) -> Self {
        self.bytes(name.as_bytes(), 0xfa).bytes(disc.as_bytes(), 0xfb)
    }

    /// Free text that changes the encoding (as [`Desc::note`]).
    #[must_use]
    pub const fn note(self, text: &str) -> Self {
        self.bytes(text.as_bytes(), 0xfc)
    }

    /// A number that changes the encoding (an array's length).
    #[must_use]
    pub const fn number(self, n: u64) -> Self {
        self.bytes(&n.to_le_bytes(), 0xfd)
    }

    /// The hash.
    #[must_use]
    pub const fn finish(self) -> u64 {
        self.0
    }
}

/// The schema hash of `T` and every type its encoding reaches, from the
/// descriptor walk (the readable form; [`Snap::SCHEMA_HASH`] is the
/// constant a snapshot stores).
pub fn schema_hash<T: Snap>() -> u64 {
    let mut s = Schema::new();
    T::schema(&mut s);
    s.hash()
}
