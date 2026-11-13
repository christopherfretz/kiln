//! Evaluation failures: the note chain clang attaches to "expression is
//! not a constant expression" (spec/oven.md "Evaluation entry points").

use std::fmt;

use kiln_source::Loc;

/// One note of a failure.
#[derive(Clone, Debug, PartialEq, Eq, kiln_snap::Snap)]
pub struct Note {
    /// Where the note points.
    pub loc: Loc,
    /// The text, in clang's wording (`read of uninitialized object is not
    /// allowed in a constant expression`, `in call to 'f(1)'`, ...).
    pub msg: String,
}

/// The starts of clang's `err_` diagnostics that the evaluator reports as
/// notes (clang `Info.FFDiag` with an error id); they print and count as
/// errors (spec/oven.md "Evaluation entry points").
const ERROR_NOTES: &[&str] =
    &["index for __builtin_shufflevector not within the bounds of the input vectors"];

impl Note {
    /// A note at `loc`.
    pub fn new(loc: Loc, msg: impl Into<String>) -> Self {
        Note { loc, msg: msg.into() }
    }

    /// True for a note clang prints at error severity.
    pub fn is_error(&self) -> bool {
        ERROR_NOTES.iter().any(|p| self.msg.starts_with(p))
    }
}

/// A signed integer operation whose result was outside its type in an
/// evaluation checking for undefined behaviour
/// ([`crate::Oven::fold_for_overflow`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Overflow {
    /// The operator.
    pub loc: Loc,
    /// The operation (its type is the one clang names), if known.
    pub e: Option<kiln_ast::ExprId>,
    /// The result wrapped to the operation's width, sign-extended.
    pub value: i128,
}

/// What kind of limit, if any, stopped the evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailKind {
    /// The expression is not a constant expression ([expr.const]/10).
    NotConstant,
    /// The step budget was exhausted (`-fconstexpr-steps`).
    StepLimit,
    /// The call-depth budget was exhausted (`-fconstexpr-depth`).
    DepthLimit,
    /// The abstract-memory budget was exhausted (`-fconstexpr-memory`).
    MemoryLimit,
    /// The value is constant but the variable's destruction is not
    /// ([dcl.constexpr]/6, "must have constant destruction"); the notes
    /// say why.
    Destruction,
}

/// Why an evaluation failed: the reason (the first note) followed by the
/// call stack (`in call to '...'`, innermost first) and supporting notes
/// (`declared here`, ...), in the order clang emits them.
///
/// A silent evaluation ([`crate::Oven::try_fold`]) produces a failure
/// without notes: nothing is rendered when no one will read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvalFailure {
    /// The kind of failure.
    pub kind: FailKind,
    /// The notes; empty for a silent evaluation.
    pub notes: Vec<Note>,
}

impl EvalFailure {
    /// A failure with no notes.
    pub fn silent(kind: FailKind) -> Self {
        EvalFailure { kind, notes: Vec::new() }
    }

    /// The reason (the first note), if rendered.
    pub fn primary(&self) -> Option<&Note> {
        self.notes.first()
    }
}

impl fmt::Display for EvalFailure {
    /// One `note: <msg>` line per note (locations are rendered by the
    /// driver's diagnostic engine, which knows the file names).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, n) in self.notes.iter().enumerate() {
            if i > 0 {
                f.write_str("\n")?;
            }
            write!(f, "note: {}", n.msg)?;
        }
        Ok(())
    }
}

impl std::error::Error for EvalFailure {}
