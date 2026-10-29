//! Evaluation failures: the note chain clang attaches to "expression is
//! not a constant expression" (spec/oven.md "Evaluation entry points").

use std::fmt;

use kiln_source::Loc;

/// One note of a failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    /// Where the note points.
    pub loc: Loc,
    /// The text, in clang's wording (`read of uninitialized object is not
    /// allowed in a constant expression`, `in call to 'f(1)'`, ...).
    pub msg: String,
}

impl Note {
    /// A note at `loc`.
    pub fn new(loc: Loc, msg: impl Into<String>) -> Self {
        Note { loc, msg: msg.into() }
    }
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
