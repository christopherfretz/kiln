//! Warning groups (`-W<group>`), with clang's names (`spec/diag.md`
//! section "Warning groups").
//!
//! A group's members are the ids whose [`DiagId::group`] is the group, the
//! members of its subgroups (transitively), and, for [`Group::Pedantic`],
//! every extension diagnostic without a group. Adding a group is a one-line
//! change to the `diag_groups!` invocation below:
//! `Variant = "clang-name" [Subgroup, ...];`.
//!
//! Group names clang knows but kiln has no member for are listed in
//! [`CLANG_GROUP_NAMES`](crate::clang_groups::CLANG_GROUP_NAMES); options
//! naming them are accepted and do nothing.

use crate::clang_groups::CLANG_GROUP_NAMES;
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
            /// Every group, in table order.
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

diag_groups! {
    All = "all" [Most];
    Most = "most" [Comment, UnknownPragmas];
    Pragmas = "pragmas" [UnknownPragmas];
    Pedantic = "pedantic" [];
    BackslashNewlineEscape = "backslash-newline-escape" [];
    InvalidPpToken = "invalid-pp-token" [];
    MacroRedefined = "macro-redefined" [];
    UnusedMacros = "unused-macros" [];
    PoundWarning = "#warnings" [];
    PoundPragmaMessage = "#pragma-messages" [];
    ExtraTokens = "extra-tokens" [];
    UnknownPragmas = "unknown-pragmas" [];
    UnknownWarningOption = "unknown-warning-option" [];
    // ----- WP-048 (parser and sema)
    UnknownEscapeSequence = "unknown-escape-sequence" [];
    Multichar = "multichar" [];
    Attributes = "attributes" [UnknownAttributes, IgnoredAttributes];
    UnknownAttributes = "unknown-attributes" [];
    IgnoredAttributes = "ignored-attributes" [];
    DuplicateDeclSpecifier = "duplicate-decl-specifier" [];
    MissingDeclarations = "missing-declarations" [];
    ExternInitializer = "extern-initializer" [];
    VlaCxxExtension = "vla-cxx-extension" [];
    BitfieldWidth = "bitfield-width" [];
    PointerArith = "pointer-arith" [];
    DivisionByZero = "division-by-zero" [];
    ShiftCountNegative = "shift-count-negative" [];
    ShiftCountOverflow = "shift-count-overflow" [];
    Unused = "unused" [UnusedValue];
    UnusedValue = "unused-value" [UnusedComparison, UnusedResult];
    UnusedComparison = "unused-comparison" [];
    UnusedResult = "unused-result" [];
    DeprecatedDeclarations = "deprecated-declarations" [];
    ReturnType = "return-type" [];
    // sema conversions (WP-049)
    ImplicitlyUnsignedLiteral = "implicitly-unsigned-literal" [];
    LiteralRange = "literal-range" [];
    Cxx11Narrowing = "c++11-narrowing" [];
    Narrowing = "narrowing" [Cxx11Narrowing];
    BuiltinMacroRedefined = "builtin-macro-redefined" [];
    Comment = "comment" [];
    C23Extensions = "c23-extensions" [];
    Cxx20Compat = "c++20-compat" [];
    DateTime = "date-time" [];
    DelimitedEscapeSequenceExtension = "delimited-escape-sequence-extension" [];
    DisabledMacroExpansion = "disabled-macro-expansion" [];
    DollarInIdentifierExtension = "dollar-in-identifier-extension" [];
    EmbeddedDirective = "embedded-directive" [];
    ExpansionToDefined = "expansion-to-defined" [];
    GnuIncludeNext = "gnu-include-next" [];
    GnuLineMarker = "gnu-line-marker" [];
    GnuZeroLineDirective = "gnu-zero-line-directive" [];
    GnuZeroVariadicMacroArguments = "gnu-zero-variadic-macro-arguments" [];
    IncludeNextAbsPath = "include-next-absolute-path" [];
    IncludeNextOutsideHeader = "include-next-outside-header" [];
    KeywordAsMacro = "keyword-macro" [];
    MalformedWarningCheck = "malformed-warning-check" [];
    NewlineEof = "newline-eof" [];
    NullCharacter = "null-character" [];
    PragmaOnceOutsideHeader = "pragma-once-outside-header" [];
    PragmaSystemHeaderOutsideHeader = "pragma-system-header-outside-header" [];
    ReservedMacroIdentifier = "reserved-macro-identifier" [];
    ReservedUserDefinedLiteral = "reserved-user-defined-literal" [];
    Trigraphs = "trigraphs" [];
    Unicode = "unicode" [];
    UnicodeWhitespace = "unicode-whitespace" [];
    VariadicMacros = "variadic-macros" [];
    // #if expressions and literals (WP-015)
    Undef = "undef" [];
    FourByteMultiChar = "four-char-constants" [];
    Cxx11LongLong = "c++11-long-long" [];
    Cxx11Extensions = "c++11-extensions" [Cxx11LongLong];
    Cxx14BinaryLiteral = "c++14-binary-literal" [];
    Cxx14Extensions = "c++14-extensions" [Cxx14BinaryLiteral];
    Cxx17Extensions = "c++17-extensions" [];
    Cxx23Extensions = "c++23-extensions" [];
}

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
    /// subgroup, or as a group-less extension of `pedantic`).
    pub fn contains(self, id: DiagId) -> bool {
        if self == Group::Pedantic && id.group().is_none() && id.class() == DiagClass::Extension {
            return true;
        }
        match id.group() {
            Some(g) => self.contains_group(g),
            None => false,
        }
    }

    fn contains_group(self, g: Group) -> bool {
        self == g || self.subgroups().iter().any(|s| s.contains_group(g))
    }

    /// The members of this group, in table order.
    pub fn members(self) -> impl Iterator<Item = DiagId> {
        DiagId::ALL.iter().copied().filter(move |&id| self.contains(id))
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

/// The members of the group named `name` for `flavor`, or `None` if
/// neither kiln nor clang knows the name (clang's `getDiagnosticsInGroup`
/// returning true).
pub(crate) fn group_members(flavor: Flavor, name: &str) -> Option<Vec<DiagId>> {
    if let Some(g) = Group::from_name(name) {
        return Some(g.members().filter(|&id| flavor.matches(id)).collect());
    }
    CLANG_GROUP_NAMES.binary_search(&name).is_ok().then(Vec::new)
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
