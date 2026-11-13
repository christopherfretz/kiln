//! The schema hash (spec/snap.md "Schema hash").
//!
//! Every [`Snap`] type describes itself once in a [`Schema`]: its
//! `std::any::type_name`, its kind, its fields (name and field type name,
//! in order) or variants (name and explicit discriminant), and the
//! attributes that change its encoding. [`schema_hash`] hashes the sorted
//! descriptors of everything reachable from a root. A snapshot is only
//! read by a build whose hash matches the writer's, so a struct-layout
//! change anywhere below the root is a clean refusal, not a misread.

use std::any::{TypeId, type_name};
use std::collections::HashSet;
use std::fmt;
use std::fmt::Write as _;

use crate::Snap;

/// The descriptor of one type: built with [`Desc::new`] and the field and
/// variant methods, in declaration order, then passed to
/// [`Schema::register`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Desc(String);

impl Desc {
    /// A descriptor of kind `kind` (`"struct"`, `"enum"`, `"u32"`,
    /// `"vec"`, `"root"`, ...).
    pub fn new(kind: &str) -> Self {
        Desc(kind.to_owned())
    }

    /// An encoded field `name` of type `T`.
    #[must_use]
    pub fn field<T: ?Sized>(mut self, name: &str) -> Self {
        let _ = write!(self.0, " {name}:{}", type_name::<T>());
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
        Schema { seen: HashSet::new(), descs: Vec::new() }
    }

    /// Registers `T` with descriptor `desc`. Returns `true` the first time
    /// for `T`: the caller then calls `schema` on each encoded field type.
    /// Registering before recursing makes recursive types terminate.
    pub fn register<T: ?Sized + 'static>(&mut self, desc: Desc) -> bool {
        if !self.seen.insert(TypeId::of::<T>()) {
            return false;
        }
        let Desc(text) = desc;
        self.descs.push(format!("{} = {text}", type_name::<T>()));
        true
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
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for d in self.descriptors() {
            for &b in d.as_bytes().iter().chain(&[0xff]) {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        h
    }
}

/// The schema hash of `T` and every type its encoding reaches.
pub fn schema_hash<T: Snap>() -> u64 {
    let mut s = Schema::new();
    T::schema(&mut s);
    s.hash()
}
