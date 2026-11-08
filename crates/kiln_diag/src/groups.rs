//! Warning groups (`-W<group>`), with clang's names (`spec/diag.md`
//! sections 3, 2.4).
//!
//! A group's members are the ids whose [`DiagId::group`] is the group, the
//! members of its subgroups (transitively), and, for [`Group::Pedantic`],
//! every extension diagnostic without a group. The rows,
//! `Variant = "clang-name" [Subgroup, ...];`, live under `src/groups/`:
//! groups with subgroups (the umbrellas `-Wall`, `-Wmost`, `-Wgnu`, ...)
//! and `Pedantic` in `umbrellas.rs`, every other group in the file of the
//! id prefix that uses it (`groups/sema.rs`, ...) or `misc.rs`, each sorted
//! by name. `build.rs` joins them into the `diag_group_table!` macro, which
//! passes every row to `diag_groups!` below. To add a group, add its row
//! there, or to the tail block at the end of this file and run `cargo xtask
//! diag-split`.
//!
//! Group names clang knows but kiln has no member for are listed in
//! [`CLANG_GROUP_NAMES`](crate::clang_groups::CLANG_GROUP_NAMES) (and, at
//! `-fclang-compat=22`, [`CLANG_POST19_GROUP_NAMES`]); options naming them
//! are accepted and do nothing.

use crate::clang_groups::{CLANG_GROUP_NAMES, CLANG_POST19_GROUP_NAMES};
use crate::ids::{DiagClass, DiagId};

macro_rules! diag_groups {
    ($( $variant:ident = $name:literal [$($sub:ident),*]; )*) => {
        /// A warning group kiln has at least a declaration for.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Group {
            $(
                #[doc = concat!("`-W", $name, "`")]
                $variant,
            )*
        }

        impl Group {
            /// Every group, in table order (the area files in file-name
            /// order, each sorted by name, then the tail).
            pub const ALL: &'static [Group] = &[$(Group::$variant),*];

            /// The option name (`-W<name>`), as clang spells it.
            pub const fn name(self) -> &'static str {
                match self { $(Group::$variant => $name,)* }
            }

            /// Direct subgroups.
            pub const fn subgroups(self) -> &'static [Group] {
                match self { $(Group::$variant => &[$(Group::$sub),*],)* }
            }
        }
    };
}

// The table: `src/groups/*.rs`, joined by `build.rs` (spec/diag.md §2.4).
include!(concat!(env!("OUT_DIR"), "/diag_group_table.rs"));

impl Group {
    /// Looks a kiln group up by option name.
    pub fn from_name(name: &str) -> Option<Group> {
        Group::ALL.iter().copied().find(|g| g.name() == name)
    }

    /// True if `name` (without `-W`) names a warning group clang 19
    /// accepts, whether or not kiln has diagnostics in it (`__has_warning`).
    pub fn is_known_name(name: &str) -> bool {
        Group::from_name(name).is_some() || CLANG_GROUP_NAMES.binary_search(&name).is_ok()
    }

    /// True if `id` is a member of this group (directly, through a
    /// subgroup, or as a group-less extension of `pedantic`), with clang
    /// 19's group tree.
    pub fn contains(self, id: DiagId) -> bool {
        self.contains_at(id, 19)
    }

    /// [`Self::contains`] with the group tree of clang `compat`
    /// ([`Self::compat_subgroups`]).
    pub fn contains_at(self, id: DiagId, compat: u32) -> bool {
        if self == Group::Pedantic && id.group().is_none() && id.class() == DiagClass::Extension {
            return true;
        }
        match id.group() {
            Some(g) => self.contains_group(g, compat),
            None => false,
        }
    }

    fn contains_group(self, g: Group, compat: u32) -> bool {
        self == g
            || self.subgroups().iter().any(|s| s.contains_group(g, compat))
            || self.compat_subgroups(compat).iter().any(|s| s.contains_group(g, compat))
    }

    /// Subgroups a group has only from a clang compat level on
    /// (`-fclang-compat=`, spec/diag.md §2.4): clang 22's `c99-designator`
    /// takes `initializer-overrides` and `reorder-init-list`, so
    /// `-Wno-c99-designator` silences both.
    pub const fn compat_subgroups(self, compat: u32) -> &'static [Group] {
        match self {
            Group::C99Designator if compat >= 22 => {
                &[Group::InitializerOverrides, Group::ReorderInitList]
            }
            _ => &[],
        }
    }

    /// The members of this group, in table order (clang 19's tree).
    pub fn members(self) -> impl Iterator<Item = DiagId> {
        self.members_at(19)
    }

    /// The members of this group with the group tree of clang `compat`.
    pub fn members_at(self, compat: u32) -> impl Iterator<Item = DiagId> {
        DiagId::ALL.iter().copied().filter(move |&id| self.contains_at(id, compat))
    }
}

/// Which diagnostics a group option addresses: `-W` options address
/// warnings and extensions, `-R` options remarks (clang's `diag::Flavor`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Flavor {
    WarningOrError,
    Remark,
}

impl Flavor {
    pub(crate) fn matches(self, id: DiagId) -> bool {
        match self {
            Flavor::WarningOrError => id.class() != DiagClass::Remark,
            Flavor::Remark => id.class() == DiagClass::Remark,
        }
    }
}

/// The members of the group named `name` for `flavor` (with clang
/// `compat`'s group tree), or `None` if
/// neither kiln nor clang knows the name (clang's `getDiagnosticsInGroup`
/// returning true).
pub(crate) fn group_members(flavor: Flavor, name: &str, compat: u32) -> Option<Vec<DiagId>> {
    if let Some(g) = Group::from_name(name) {
        return Some(g.members_at(compat).filter(|&id| flavor.matches(id)).collect());
    }
    (CLANG_GROUP_NAMES.binary_search(&name).is_ok()
        || (compat >= 22 && CLANG_POST19_GROUP_NAMES.binary_search(&name).is_ok()))
    .then(Vec::new)
}

/// The closest group name to `name` with at least one member of
/// `flavor`, as clang's `DiagnosticIDs::getNearestOption`: edit distance
/// at most `name.len()`, and a tie at the best distance suggests nothing.
pub(crate) fn nearest_group(flavor: Flavor, name: &str) -> Option<&'static str> {
    let mut groups: Vec<Group> = Group::ALL.to_vec();
    groups.sort_by_key(|g| g.name());
    let mut best: Option<&'static str> = None;
    let mut best_distance = name.len() + 1;
    for g in groups {
        let d = edit_distance(g.name(), name);
        if d > best_distance {
            continue;
        }
        if !g.members().any(|id| flavor.matches(id)) {
            continue;
        }
        if d == best_distance {
            best = None;
        } else {
            best = Some(g.name());
            best_distance = d;
        }
    }
    best
}

/// Levenshtein distance (insertions, deletions and replacements), by
/// bytes.
pub(crate) fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, &ca) in a.iter().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = if ca == cb { prev } else { 1 + prev.min(cur).min(row[j]) };
            prev = cur;
        }
    }
    row[b.len()]
}

// The tail block (spec/diag.md §2.4): rows here compile, but `cargo xtask
// diag-split --check` (run by `cargo xtask ci`) fails until `cargo xtask
// diag-split` has moved them into their area file under `src/groups/`.
diag_group_table! {
}
