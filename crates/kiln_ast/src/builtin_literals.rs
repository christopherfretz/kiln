//! The builtin-macro literal table (spec/ast.md §22, WP-2821): an opt-in
//! side table that says which builtin macro (`__LINE__`, `__FILE__`, ...)
//! a literal came from, for tooling clients that must not treat such a
//! literal as one written by value.
//!
//! Off by default: the preprocessor keeps its token table only when
//! `PpOptions::builtin_literals` is set, and the parser enters a literal
//! here only then, so a unit compiled without it has an empty table and
//! pays one branch per literal. Keyed by the literal's location (its
//! first token as the parser received it), which an instantiated copy of
//! the literal keeps.

use kiln_source::Loc;
use kiln_source::fx::FxHashMap;

use crate::ast::Ast;
use crate::expr::ExprKind;
use crate::ids::ExprId;
use crate::mem::MemRow;

/// A builtin macro whose result is a literal that depends on where, in
/// which order, when or for which target it was expanded (spec/pp.md
/// "Builtin macros"). The feature queries (`__has_include`,
/// `__has_builtin`, ...) and `_Pragma` are not recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuiltinMacro {
    /// `__LINE__` (an integer literal).
    Line,
    /// `__FILE__` (a string literal).
    File,
    /// `__FILE_NAME__` (a string literal).
    FileName,
    /// `__BASE_FILE__` (a string literal).
    BaseFile,
    /// `__COUNTER__` (an integer literal).
    Counter,
    /// `__INCLUDE_LEVEL__` (an integer literal).
    IncludeLevel,
    /// `__DATE__` (a string literal).
    Date,
    /// `__TIME__` (a string literal).
    Time,
    /// `__TIMESTAMP__` (a string literal).
    Timestamp,
    /// `__FLT_EVAL_METHOD__` (an integer literal).
    FltEvalMethod,
}

impl BuiltinMacro {
    /// Every recorded builtin, in the order of the enum.
    pub const ALL: [BuiltinMacro; 10] = [
        BuiltinMacro::Line,
        BuiltinMacro::File,
        BuiltinMacro::FileName,
        BuiltinMacro::BaseFile,
        BuiltinMacro::Counter,
        BuiltinMacro::IncludeLevel,
        BuiltinMacro::Date,
        BuiltinMacro::Time,
        BuiltinMacro::Timestamp,
        BuiltinMacro::FltEvalMethod,
    ];

    /// The macro's name (`"__LINE__"`).
    pub const fn name(self) -> &'static str {
        match self {
            BuiltinMacro::Line => "__LINE__",
            BuiltinMacro::File => "__FILE__",
            BuiltinMacro::FileName => "__FILE_NAME__",
            BuiltinMacro::BaseFile => "__BASE_FILE__",
            BuiltinMacro::Counter => "__COUNTER__",
            BuiltinMacro::IncludeLevel => "__INCLUDE_LEVEL__",
            BuiltinMacro::Date => "__DATE__",
            BuiltinMacro::Time => "__TIME__",
            BuiltinMacro::Timestamp => "__TIMESTAMP__",
            BuiltinMacro::FltEvalMethod => "__FLT_EVAL_METHOD__",
        }
    }

    /// The builtin named `name`, if it is one of [`Self::ALL`].
    pub fn from_name(name: &str) -> Option<BuiltinMacro> {
        BuiltinMacro::ALL.into_iter().find(|b| b.name() == name)
    }
}

/// The table: literal location → builtin (§22.2).
#[derive(Clone, Debug, Default)]
pub struct BuiltinLiterals {
    map: FxHashMap<Loc, BuiltinMacro>,
}

impl BuiltinLiterals {
    /// Enters the literal at `loc` (its first token's location).
    pub fn insert(&mut self, loc: Loc, b: BuiltinMacro) {
        self.map.insert(loc, b);
    }

    /// The builtin of the literal at `loc`.
    pub fn get(&self, loc: Loc) -> Option<BuiltinMacro> {
        self.map.get(&loc).copied()
    }

    /// The number of literals entered.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether no literal is entered.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Every entry, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = (Loc, BuiltinMacro)> + '_ {
        self.map.iter().map(|(&l, &b)| (l, b))
    }

    /// The `-fkiln-memory-report` row, when anything is entered.
    pub fn mem_row(&self) -> Option<MemRow> {
        if self.map.is_empty() {
            return None;
        }
        let n = self.map.len() as u64;
        let bytes = self.map.capacity() as u64 * (size_of::<(Loc, BuiltinMacro)>() as u64 + 1);
        Some(MemRow::new("builtin_literals", n, bytes, bytes))
    }
}

impl Ast {
    /// The builtin macro the literal `e` came from (§22.3): `e` an
    /// `IntegerLiteral`, `FloatingLiteral`, `CharacterLiteral`,
    /// `StringLiteral` (any of its concatenated pieces) or
    /// `UserDefinedLiteral` made from a builtin's result, directly,
    /// through a user macro or a macro argument, or pasted (`##`) or
    /// stringized (`#`) from one. `None` for any other expression, a
    /// literal written in the source, and every literal of a unit
    /// compiled without `PpOptions::builtin_literals`.
    pub fn literal_builtin(&self, e: ExprId) -> Option<BuiltinMacro> {
        if self.builtin_literals.is_empty() {
            return None;
        }
        let n = self.exprs.get(e);
        match n.kind() {
            ExprKind::IntegerLiteral
            | ExprKind::FloatingLiteral
            | ExprKind::CharacterLiteral
            | ExprKind::StringLiteral
            | ExprKind::UserDefinedLiteral => self.builtin_literals.get(n.loc()),
            _ => None,
        }
    }
}
