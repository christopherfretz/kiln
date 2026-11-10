//! The reflection dispatch (spec/reflection.md §3): the [`MetaOp`]
//! families, their purity classes, and the types of the single bridge
//! entry point [`crate::SemaBridge::meta`].
//!
//! Each family is one file holding one `meta_family!` table: an op's
//! source name, its signature, its purity class and the work package that
//! owns it. A later work package adds rows (or a file and a [`MetaOp`]
//! variant), never a signature change. The VM half is `vm/meta.rs`.

use kiln_ast::{ConstValueId, DeclId, ReflectionId};
use kiln_source::Loc;

use crate::Note;
use crate::ty::OTy;
use crate::vm::Fault;

mod access;
mod annot;
pub(crate) mod define;
mod ext;
pub(crate) mod intern;
pub mod materialize;
mod member;
pub mod member_cache;
mod query;
mod templ;
mod traits;
pub(crate) mod value;

pub use access::AccessOp;
pub use annot::AnnotOp;
pub use define::{DefineOp, Dms, OptionsPlan, RawOptions};
pub use ext::ExtOp;
pub use member::MemberOp;
pub use query::QueryOp;
pub use templ::TemplOp;
pub use traits::TraitOp;
pub use value::ValueOp;

/// The most operands a reflection intrinsic takes (a call with more is
/// not compiled; no op in the tables has more than three).
pub const MAX_META_ARGS: usize = 8;

/// What an op's result depends on (spec/reflection.md §3.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Purity {
    /// A pure function of the operands: a call that reached only such
    /// ops may be memoized.
    Stable,
    /// Depends on the evaluation point (what the translation unit has
    /// declared or completed so far): the calling evaluation is impure.
    PointDependent,
    /// Changes the program: impure, and a silent failure in a trial (a
    /// fold, `__builtin_constant_p`, a speculative checkpoint).
    Mutating,
}

/// Generates one family's op enum and its table accessors from rows of
/// `Variant = ("source name", "signature", Purity, owner WP);`. The
/// signature has one letter per parameter: `i` a `std::meta::info`, `x`
/// anything else. The variant order is the op's code within the family:
/// rows are only ever appended (codes are not stored outside a run, but
/// appending keeps diffs local).
macro_rules! meta_family {
    (
        $(#[$m:meta])*
        pub enum $E:ident {
            $( $(#[$vm:meta])* $V:ident = ($name:literal, $sig:literal, $pur:ident, $wp:literal); )*
        }
    ) => {
        $(#[$m])*
        #[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
        pub enum $E {
            $( $(#[$vm])* $V, )*
        }

        impl $E {
            /// Every op of the family, in code order.
            pub const ALL: &'static [Self] = &[$(Self::$V),*];

            /// The name the op's function is declared with (a member of
            /// a class in `std::meta` is `class::name`).
            pub const fn name(self) -> &'static str {
                match self { $(Self::$V => $name,)* }
            }

            /// The declared parameters: `i` per `info`, `x` per other.
            pub const fn sig(self) -> &'static str {
                match self { $(Self::$V => $sig,)* }
            }

            /// The op's purity class.
            pub const fn purity(self) -> $crate::meta::Purity {
                match self { $(Self::$V => $crate::meta::Purity::$pur,)* }
            }

            /// The work package that implements (or implemented) the op.
            pub const fn owner(self) -> u16 {
                match self { $(Self::$V => $wp,)* }
            }
        }
    };
}
pub(crate) use meta_family;

/// A reflection intrinsic: a call of a function the bridge recognizes by
/// its declaration ([`crate::SemaBridge::meta_intrinsic`]) compiles to
/// `Op::Builtin` (`bi::META`) and runs through [`crate::SemaBridge::meta`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum MetaOp {
    /// Entity queries and names ([meta.reflection.queries],
    /// [meta.reflection.names]).
    Query(QueryOp),
    /// Type traits ([meta.reflection.traits]).
    Trait(TraitOp),
    /// Member enumeration and layout ([meta.reflection.member.queries],
    /// [meta.reflection.layout]).
    Member(MemberOp),
    /// Values and static storage ([meta.reflection.extract],
    /// [meta.reflection.result], [meta.define.static]).
    Value(ValueOp),
    /// `data_member_spec` and `define_aggregate`
    /// ([meta.reflection.define.aggregate]).
    Define(DefineOp),
    /// Annotation queries ([meta.reflection.annotation]).
    Annot(AnnotOp),
    /// Access contexts ([meta.reflection.access.context],
    /// [meta.reflection.access.queries]).
    Access(AccessOp),
    /// Templates and substitution ([meta.reflection.substitute]).
    Template(TemplOp),
    /// kiln extensions and the spike's stand-ins (`ext.rs`).
    Ext(ExtOp),
}

/// Applies `$f` to the family op inside a [`MetaOp`].
macro_rules! each_family {
    ($op:expr, $x:ident => $f:expr) => {
        match $op {
            MetaOp::Query($x) => $f,
            MetaOp::Trait($x) => $f,
            MetaOp::Member($x) => $f,
            MetaOp::Value($x) => $f,
            MetaOp::Define($x) => $f,
            MetaOp::Annot($x) => $f,
            MetaOp::Access($x) => $f,
            MetaOp::Template($x) => $f,
            MetaOp::Ext($x) => $f,
        }
    };
}

impl MetaOp {
    /// The declared name (see the family tables).
    pub fn name(self) -> &'static str {
        each_family!(self, o => o.name())
    }

    /// The declared parameters (`i` per `info`, `x` per other).
    pub fn sig(self) -> &'static str {
        each_family!(self, o => o.sig())
    }

    /// The purity class (spec/reflection.md §3.3).
    pub fn purity(self) -> Purity {
        each_family!(self, o => o.purity())
    }

    /// The owning work package.
    pub fn owner(self) -> u16 {
        each_family!(self, o => o.owner())
    }

    /// Whether the op is declared in `std::meta` (else at global scope:
    /// the kiln extensions).
    pub fn in_std_meta(self) -> bool {
        !matches!(self, MetaOp::Ext(ExtOp::DefineMember))
    }

    /// Every op, family by family.
    pub fn all() -> impl Iterator<Item = MetaOp> {
        let q = QueryOp::ALL.iter().map(|&o| MetaOp::Query(o));
        let t = TraitOp::ALL.iter().map(|&o| MetaOp::Trait(o));
        let m = MemberOp::ALL.iter().map(|&o| MetaOp::Member(o));
        let v = ValueOp::ALL.iter().map(|&o| MetaOp::Value(o));
        let d = DefineOp::ALL.iter().map(|&o| MetaOp::Define(o));
        let a = AnnotOp::ALL.iter().map(|&o| MetaOp::Annot(o));
        let c = AccessOp::ALL.iter().map(|&o| MetaOp::Access(o));
        let p = TemplOp::ALL.iter().map(|&o| MetaOp::Template(o));
        let e = ExtOp::ALL.iter().map(|&o| MetaOp::Ext(o));
        q.chain(t).chain(m).chain(v).chain(d).chain(a).chain(c).chain(p).chain(e)
    }

    /// The op declared as `name` with parameters `info_params` (`true`
    /// per `info` parameter), in `std::meta` if `in_std_meta`.
    pub fn lookup(name: &str, info_params: &[bool], in_std_meta: bool) -> Option<MetaOp> {
        let sig_is = |s: &str| {
            s.len() == info_params.len()
                && s.bytes().zip(info_params).all(|(c, &i)| (c == b'i') == i)
        };
        Self::all()
            .find(|op| op.in_std_meta() == in_std_meta && op.name() == name && sig_is(op.sig()))
    }

    /// The op's code in the instruction's `sub` field: family << 8 | index.
    pub(crate) fn code(self) -> u32 {
        // A family enum's discriminants are its row indices.
        let (fam, i) = match self {
            MetaOp::Query(o) => (0, o as u32),
            MetaOp::Trait(o) => (1, o as u32),
            MetaOp::Member(o) => (2, o as u32),
            MetaOp::Value(o) => (3, o as u32),
            MetaOp::Define(o) => (4, o as u32),
            MetaOp::Annot(o) => (5, o as u32),
            MetaOp::Access(o) => (6, o as u32),
            MetaOp::Template(o) => (7, o as u32),
            MetaOp::Ext(o) => (8, o as u32),
        };
        fam << 8 | i
    }

    /// The op of `code`.
    pub(crate) fn from_code(code: u32) -> Option<MetaOp> {
        let i = (code & 0xff) as usize;
        Some(match code >> 8 {
            0 => MetaOp::Query(*QueryOp::ALL.get(i)?),
            1 => MetaOp::Trait(*TraitOp::ALL.get(i)?),
            2 => MetaOp::Member(*MemberOp::ALL.get(i)?),
            3 => MetaOp::Value(*ValueOp::ALL.get(i)?),
            4 => MetaOp::Define(*DefineOp::ALL.get(i)?),
            5 => MetaOp::Annot(*AnnotOp::ALL.get(i)?),
            6 => MetaOp::Access(*AccessOp::ALL.get(i)?),
            7 => MetaOp::Template(*TemplOp::ALL.get(i)?),
            8 => MetaOp::Ext(*ExtOp::ALL.get(i)?),
            _ => return None,
        })
    }

    /// The name as a note spells it (`std::meta::type_of`).
    pub fn display(self) -> String {
        if self.in_std_meta() { format!("std::meta::{}", self.name()) } else { self.name().into() }
    }
}

/// Where and how an intrinsic is evaluated (spec/reflection.md §3).
#[derive(Clone, Copy, Debug)]
pub struct MetaCtx {
    /// The scope of the evaluation point (`access_context::current()`):
    /// the function whose body holds the call (its lexical scope; a
    /// default argument is compiled at its call site), `None` at the
    /// top level of an evaluation (the bridge's current context).
    pub scope: Option<DeclId>,
    /// The call's location (the position of the evaluation point, and
    /// where failure notes point).
    pub loc: Loc,
    /// The evaluation is plainly constant-evaluated: a consteval
    /// block's ([expr.const.core]/2.32, the only context in which
    /// `define_aggregate` may inject); [`crate::Oven::plain`]
    /// (spec/consteval-blocks.md §3).
    pub plainly: bool,
    /// The evaluation is a trial (a fold, `__builtin_constant_p`, a
    /// speculative checkpoint): a `Mutating` op never reaches the bridge.
    pub trial: bool,
    /// The element type of the call's last pointer argument (what
    /// [`crate::Oven::meta_c_string`] reads); for a value op given a
    /// range ([`materialize::Shape::Elems`]), the range's element type.
    pub(crate) elem: OTy,
    /// The intrinsic's declaration: the specialization whose signature
    /// gives a function template's `T` (`extract<T>`, `reflect_constant`;
    /// WP-479).
    pub callee: Option<DeclId>,
    /// The first register above the calling frame's (where a global
    /// thawed for [`crate::Oven::meta_freeze_at`] is evaluated).
    pub(crate) top: u32,
}

/// An intrinsic's result, written by the bridge.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MetaOut {
    /// Nothing written (a bridge bug when the op succeeded).
    #[default]
    Unset,
    /// A scalar of the call's result type (`bool`, integers).
    Scalar(u128),
    /// A `std::meta::info`.
    Info(ReflectionId),
    /// A frozen pointer value, thawed as the call's (pointer) result type.
    /// For a value op (WP-479), any frozen value of the result type: a
    /// scalar is thawed into the result register, a class through the
    /// site's [`materialize::Recipe::Whole`] into the result object.
    Frozen(ConstValueId),
    /// The value of the object a frozen pointer designates, read as the
    /// call's result type (`extract<T>` of an object or a variable).
    Deref(ConstValueId),
    /// A string view (`materialize::Shape::Str`): a frozen pointer to the
    /// characters of a static string object, and its length.
    Str(ConstValueId, u64),
    /// A `source_location` (`materialize::Shape::SrcLoc`): the frozen
    /// `const __impl *` it holds, null for `source_location{}`.
    SrcLoc(Option<ConstValueId>),
    /// Two scalars (`materialize::Shape::Pair`).
    Pair(u128, u128),
    /// A `vector<info>` (`materialize::Shape::Vector`) of the first `n`
    /// reflections the bridge wrote to [`crate::Oven::meta_infos`].
    Infos(u32),
}

/// Why an intrinsic failed.
#[derive(Clone, Debug)]
pub enum MetaErrorKind {
    /// Not a constant expression: the notes say why (none: silently).
    Failed,
    /// The op exists in the tables but has no body yet.
    NotImplemented {
        /// The owning work package.
        wp: u16,
    },
    /// An evaluator fault (reading an operand from Oven memory).
    Fault(OvenFault),
}

/// An evaluator fault carried through a bridge call unchanged.
#[derive(Clone, Debug)]
pub struct OvenFault(pub(crate) Fault);

/// A failed intrinsic (spec/reflection.md §8): the first note is the
/// reason, reported at its location, the rest support it.
#[derive(Clone, Debug)]
pub struct MetaError {
    /// The notes (empty for a silent failure or a fault).
    pub notes: Vec<Note>,
    /// What kind of failure.
    pub kind: MetaErrorKind,
}

impl MetaError {
    /// A failure explained by `notes`.
    pub fn failed(notes: Vec<Note>) -> Self {
        MetaError { notes, kind: MetaErrorKind::Failed }
    }

    /// A failure with one note.
    pub fn note(loc: Loc, msg: impl Into<String>) -> Self {
        Self::failed(vec![Note::new(loc, msg)])
    }

    /// A silent failure (a trial; the manifestly constant-evaluated
    /// evaluation, if any, reports).
    pub fn silent() -> Self {
        Self::failed(Vec::new())
    }

    /// An argument the compiled call cannot have produced (an `info`
    /// operand that is no reflection, a missing operand): the evaluator's
    /// invalid-program fault.
    pub fn invalid() -> Self {
        MetaError { notes: Vec::new(), kind: MetaErrorKind::Fault(OvenFault(Fault::Invalid)) }
    }

    /// `op` is not implemented yet: the note names its owning WP.
    pub fn not_implemented(op: MetaOp, loc: Loc) -> Self {
        let wp = op.owner();
        MetaError {
            notes: vec![Note::new(
                loc,
                format!(
                    "kiln: reflection function '{}' is not implemented yet (WP-{wp})",
                    op.display()
                ),
            )],
            kind: MetaErrorKind::NotImplemented { wp },
        }
    }
}

/// Argument `i` of an intrinsic call as the `info` it holds.
///
/// # Errors
/// [`MetaError::invalid`] if there is no such argument or it holds no
/// reflection.
pub fn info_arg(args: &[u128], i: usize) -> Result<ReflectionId, MetaError> {
    args.get(i).and_then(|&v| ReflectionId::from_bits(v)).ok_or_else(MetaError::invalid)
}

impl From<Vec<Note>> for MetaError {
    fn from(notes: Vec<Note>) -> Self {
        Self::failed(notes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_and_names_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for op in MetaOp::all() {
            assert_eq!(MetaOp::from_code(op.code()), Some(op), "{op:?}");
            assert!(op.code() >> 8 <= 8 && op.code() & 0xff < 0xff, "{op:?}");
            assert!(op.sig().bytes().all(|c| c == b'i' || c == b'x'), "{op:?}");
            assert!(op.sig().len() <= MAX_META_ARGS, "{op:?}");
            assert!((470..=485).contains(&op.owner()), "{op:?}");
            assert!(seen.insert((op.name(), op.sig())), "duplicate {op:?}");
        }
    }

    /// `gcc16_meta_names.txt`: `(name, intrinsic)` per listed function.
    fn gcc16_names() -> Vec<(&'static str, bool)> {
        include_str!("gcc16_meta_names.txt")
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .map(|l| {
                let (name, how) = l.split_once(' ').expect("`<name> intrinsic|library`");
                assert!(matches!(how, "intrinsic" | "library"), "{l}");
                (name, how == "intrinsic")
            })
            .collect()
    }

    /// The row names, unqualified (`access_context::current` is `current`).
    fn row_names() -> std::collections::BTreeSet<&'static str> {
        MetaOp::all().map(|op| op.name().rsplit("::").next().unwrap_or_default()).collect()
    }

    /// The rows that are kiln's, not `<meta>`'s (spec/reflection.md §1.3).
    const EXTENSIONS: &[&str] = &["name_of"];

    /// The name table (WP-484): every intrinsic GCC 16.1's `<meta>`
    /// declares has a row (so a call answers or says "not implemented yet
    /// (WP-NNN)"), no function the header defines has one, and every row
    /// that is not a kiln extension is a `<meta>` intrinsic.
    #[test]
    fn rows_cover_gcc16_meta() {
        let rows = row_names();
        let names = gcc16_names();
        for &(name, intrinsic) in &names {
            assert_eq!(rows.contains(name), intrinsic, "`{name}`: row present = {}", !intrinsic);
        }
        for row in rows {
            let listed = names.iter().any(|&(n, i)| n == row && i);
            assert!(
                listed || row.starts_with("__kiln_") || EXTENSIONS.contains(&row),
                "row `{row}` is neither a <meta> intrinsic nor a kiln extension"
            );
        }
    }

    /// The `consteval` functions of a header, as `gcc16_meta_names.txt`
    /// lists them: the identifier before each `consteval` declarator's
    /// `(`, and whether some declaration of it has a body.
    fn consteval_functions(header: &str) -> std::collections::BTreeMap<String, bool> {
        let mut text = String::with_capacity(header.len());
        let mut rest = header;
        while let Some(i) = rest.find(['/']) {
            text.push_str(&rest[..i]);
            let after = &rest[i..];
            if after.starts_with("//") {
                rest = after.find('\n').map_or("", |e| &after[e..]);
            } else if after.starts_with("/*") {
                rest = after.find("*/").map_or("", |e| &after[e + 2..]);
            } else {
                text.push('/');
                rest = &after[1..];
            }
        }
        text.push_str(rest);
        let mut out = std::collections::BTreeMap::<String, bool>::new();
        for (at, _) in text.match_indices("consteval") {
            let tail = &text[at + "consteval".len()..];
            let Some(open) = tail.find('(') else { break };
            let head = &tail[..open];
            if head.contains([';', '{', '}']) {
                continue;
            }
            let head = head.trim_end();
            let start = head
                .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '~'))
                .map_or(0, |i| i + 1);
            let name = &head[start..];
            if name.is_empty()
                || name == "operator"
                || name.starts_with(|c: char| c.is_ascii_digit())
            {
                continue;
            }
            let mut depth = 0usize;
            let mut close = open;
            for (i, c) in tail[open..].char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            close = open + i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let after = &tail[close + 1..];
            let end = after.find([';', '{']).unwrap_or(after.len());
            let body = after[end..].starts_with('{')
                || after[..end].contains("= default")
                || after[..end].contains("= delete");
            *out.entry(name.to_owned()).or_default() |= body;
        }
        out
    }

    /// With `KILN_GCC16_ROOT` set (an unpacked GCC 16.1 image;
    /// docs/VALIDATION.md "GCC 16.1 header sweep"), the checked-in list
    /// is exactly the header's, so a new GCC header fails loudly.
    #[test]
    fn gcc16_meta_names_match_the_header() {
        let Some(root) = std::env::var_os("KILN_GCC16_ROOT") else { return };
        let path = std::path::Path::new(&root).join("usr/local/include/c++/16.1.0/meta");
        let Ok(header) = std::fs::read_to_string(&path) else { return };
        let header: Vec<(String, bool)> =
            consteval_functions(&header).into_iter().map(|(n, body)| (n, !body)).collect();
        let listed: Vec<(String, bool)> =
            gcc16_names().into_iter().map(|(n, i)| (n.to_owned(), i)).collect();
        assert_eq!(listed, header, "regenerate gcc16_meta_names.txt from {}", path.display());
    }

    #[test]
    fn lookup_matches_name_and_signature() {
        let t = MetaOp::lookup("type_of", &[true], true);
        assert_eq!(t, Some(MetaOp::Query(QueryOp::TypeOf)));
        assert_eq!(MetaOp::lookup("type_of", &[false], true), None);
        assert_eq!(MetaOp::lookup("type_of", &[true, true], true), None);
        assert_eq!(MetaOp::lookup("type_of", &[true], false), None);
        let d = MetaOp::lookup("__kiln_define_member", &[true, true, false], false);
        assert_eq!(d, Some(MetaOp::Ext(ExtOp::DefineMember)));
        assert_eq!(MetaOp::Query(QueryOp::TypeOf).purity(), Purity::Stable);
        assert_eq!(MetaOp::Member(MemberOp::MembersOf).purity(), Purity::PointDependent);
        assert_eq!(MetaOp::Ext(ExtOp::DefineMember).purity(), Purity::Mutating);
    }
}
