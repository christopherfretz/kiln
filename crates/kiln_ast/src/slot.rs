//! Write-once slots and cold side maps (spec/ast.md §2.4, §4.2).
//!
//! The mutation discipline of §2.4 is enforced by API: every late-bound
//! slot has a setter that refuses a second write with [`AlreadySet`]
//! (callers that consider a second write a bug `expect` it, which panics
//! with the slot's name). Cold data (fields used by few nodes of a kind)
//! lives in [`ColdMap`]s, which are insert-only.
#![warn(missing_docs)]

use std::collections::hash_map::Entry;
use std::fmt;
use std::hash::Hash;

use kiln_source::fx::FxHashMap;

use crate::mem::MemRow;

/// A second write to a write-once slot (spec/ast.md §2.4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AlreadySet {
    /// The slot, e.g. `"FunctionData.body"`.
    pub slot: &'static str,
    /// The raw value already stored.
    pub existing: u32,
}

impl fmt::Display for AlreadySet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "write-once slot {} already set (to raw value {})", self.slot, self.existing)
    }
}

impl std::error::Error for AlreadySet {}

/// Stores `new` into an `Option` slot that must still be empty.
pub(crate) fn set_once<T: Copy>(
    slot: &mut Option<T>,
    new: T,
    name: &'static str,
    raw: impl Fn(T) -> u32,
) -> Result<(), AlreadySet> {
    if let Some(old) = *slot {
        return Err(AlreadySet { slot: name, existing: raw(old) });
    }
    *slot = Some(new);
    Ok(())
}

/// An insert-only sparse map (`FxHashMap`) for cold per-node data.
#[derive(Clone, Debug)]
pub struct ColdMap<K, V> {
    name: &'static str,
    map: FxHashMap<K, V>,
}

impl<K: Copy + Eq + Hash, V> ColdMap<K, V> {
    /// An empty map whose writes report `name` on conflicts.
    pub fn new(name: &'static str) -> Self {
        Self { name, map: FxHashMap::default() }
    }

    /// The value for `k`.
    pub fn get(&self, k: K) -> Option<&V> {
        self.map.get(&k)
    }

    /// True if `k` has a value.
    pub fn contains(&self, k: K) -> bool {
        self.map.contains_key(&k)
    }

    /// Stores `v` for `k`; refuses if `k` already has a value.
    pub fn insert(&mut self, k: K, v: V) -> Result<(), AlreadySet> {
        match self.map.entry(k) {
            Entry::Occupied(_) => Err(AlreadySet { slot: self.name, existing: 0 }),
            Entry::Vacant(e) => {
                e.insert(v);
                Ok(())
            }
        }
    }

    /// Replaces the value of `k`, which must already have one: for data
    /// only final once more is known (a source-location value's function
    /// name, final once its function's return type is deduced). `false`
    /// (and no write) if `k` has no value.
    pub fn refine(&mut self, k: K, v: V) -> bool {
        match self.map.get_mut(&k) {
            Some(slot) => {
                *slot = v;
                true
            }
            None => false,
        }
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// True if there are no entries.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// The entries, in unspecified order.
    pub fn iter(&self) -> impl Iterator<Item = (K, &V)> {
        self.map.iter().map(|(k, v)| (*k, v))
    }

    /// The live bytes of [`Self::mem_row`], allocating nothing.
    pub fn live_bytes(&self) -> u64 {
        crate::mem::map_live_bytes(&self.map)
    }

    /// The memory row of this map (named after the map).
    pub fn mem_row(&self) -> MemRow {
        MemRow::of_map(self.name, &self.map)
    }
}

/// The name of every [`ColdMap`] of the AST: a decoded map gets its
/// `&'static str` back from here (spec/ast.md "Snapshots"). A map whose
/// name is missing still encodes; decoding it is an error, which the
/// round trip of an empty `Ast` in the snapshot tests reports.
const COLD_MAP_NAMES: &[&str] = &[
    "alignment",
    "annot_bases",
    "annot_decls",
    "array_trait",
    "asm_label",
    "begin_loc",
    "binding_decomp",
    "binding_expr",
    "binding_var",
    "builtin_id",
    "cast_written",
    "compound_literal",
    "contract_sites",
    "contracts",
    "conversion_spec_target",
    "ctor_inits",
    "ctor_shadow_nominated",
    "decl_attrs",
    "declarator_end",
    "decomp_bindings",
    "default_arg_rewritten",
    "default_imm",
    "delete_message",
    "deprecation",
    "designators",
    "dtor_delete",
    "dtor_delete_this",
    "end_loc",
    "explicit_expr",
    "expr_begin",
    "expr_nns",
    "expr_targs",
    "friends",
    "gnu_lib_twin",
    "hidden_shadow",
    "inherited_ctor",
    "inheriting_ctor",
    "injection_point",
    "instantiated_from",
    "key_function",
    "lexical_ctx",
    "lib_builtin",
    "member_extra",
    "member_like_friend",
    "member_op_loc",
    "offsetof_pattern",
    "overrides",
    "pack_index_origin",
    "pseudo_dtor_first",
    "pseudo_dtor_name",
    "record_pack",
    "requires_clause",
    "size_of_pack_origin",
    "source_loc",
    "splice_args",
    "syntactic_list",
    "template_defaults",
    "template_inherited_defaults",
    "template_kw",
    "trailing_requires",
    "trait_rhs_loc",
    "ttp_owner",
    "typedef_for_anon",
    "union_field",
];

/// A cold map is its name and its entries (spec/ast.md "Snapshots"): the
/// name is written as a string and read back as the matching entry of
/// [`COLD_MAP_NAMES`].
impl<K: 'static, V: 'static> kiln_snap::Snap for ColdMap<K, V>
where
    FxHashMap<K, V>: kiln_snap::Snap,
{
    fn put(&self, w: &mut kiln_snap::Writer) {
        let ColdMap { name, map } = self;
        w.str(name);
        map.put(w);
    }

    fn get(r: &mut kiln_snap::Reader<'_>) -> Result<Self, kiln_snap::SnapError> {
        let written = r.str().map_err(|e| e.within("ColdMap.name"))?;
        let Some(&name) = COLD_MAP_NAMES.iter().find(|&&n| n == written) else {
            let why = format!("unknown cold map name `{written}`");
            return Err(r.invalid::<Self>(why));
        };
        let map = r.get().map_err(|e| e.within("ColdMap.map"))?;
        Ok(ColdMap { name, map })
    }

    const SCHEMA_HASH: u64 = kiln_snap::SchemaHash::new("root", "ColdMap")
        .note("ColdMap: name as a string, then the map")
        .field::<String>("name")
        .field::<FxHashMap<K, V>>("map")
        .finish();
    fn schema(s: &mut kiln_snap::Schema) {
        let d = kiln_snap::Desc::new("root")
            .hash(Self::SCHEMA_HASH)
            .note("ColdMap: name as a string, then the map")
            .field::<String>("name")
            .field::<FxHashMap<K, V>>("map");
        if s.register::<Self>(d) {
            String::schema(s);
            <FxHashMap<K, V> as kiln_snap::Snap>::schema(s);
        }
    }
}
