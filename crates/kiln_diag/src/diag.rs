//! The `Diag` builder and the emitted `Diagnostic` record (`spec/diag.md`
//! sections "Building" and "Diagnostic records").

use std::fmt::Display;

use kiln_source::{Loc, Span};

use crate::engine::DiagEngine;
use crate::ids::DiagId;
use crate::template;

/// The level a diagnostic is rendered with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// A note attached to another diagnostic.
    Note,
    /// A remark.
    Remark,
    /// A warning.
    Warning,
    /// An error.
    Error,
    /// A fatal error.
    Fatal,
}

impl Level {
    /// The text clang prints before the message, without `": "`.
    pub const fn text_name(self) -> &'static str {
        match self {
            Level::Note => "note",
            Level::Remark => "remark",
            Level::Warning => "warning",
            Level::Error => "error",
            Level::Fatal => "fatal error",
        }
    }

    /// The `severity` value of JSON output.
    pub const fn json_name(self) -> &'static str {
        match self {
            Level::Note => "note",
            Level::Remark => "remark",
            Level::Warning => "warning",
            Level::Error => "error",
            Level::Fatal => "fatal",
        }
    }
}

/// A secondary span: underlined with `~` in text output, carried with its
/// label in JSON output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    /// The highlighted range.
    pub span: Span,
    /// Free text describing the range (JSON only; may be empty).
    pub text: String,
}

/// A suggested edit: replace `span` by `text`. Stored and rendered (the
/// insertion line under the caret, JSON `fixits`); never applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixIt {
    /// The range to remove; empty for a pure insertion.
    pub span: Span,
    /// The code to insert.
    pub text: String,
}

impl FixIt {
    /// Insert `text` at `at`.
    pub fn insert(at: Loc, text: impl Into<String>) -> FixIt {
        FixIt { span: Span::empty(at), text: text.into() }
    }

    /// Replace `span` by `text`.
    pub fn replace(span: Span, text: impl Into<String>) -> FixIt {
        FixIt { span, text: text.into() }
    }

    /// Remove `span`.
    pub fn remove(span: Span) -> FixIt {
        FixIt { span, text: String::new() }
    }
}

/// The message of a note added with [`Diag::note`]: an id taking no
/// arguments, or free text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoteMsg {
    /// A note id with no arguments.
    Id(DiagId),
    /// Free text.
    Text(String),
}

impl From<DiagId> for NoteMsg {
    fn from(id: DiagId) -> Self {
        NoteMsg::Id(id)
    }
}

impl From<&str> for NoteMsg {
    fn from(s: &str) -> Self {
        NoteMsg::Text(s.to_string())
    }
}

impl From<String> for NoteMsg {
    fn from(s: String) -> Self {
        NoteMsg::Text(s)
    }
}

/// A diagnostic under construction.
///
/// ```ignore
/// Diag::new(DiagId::pp_macro_redefined, name_span)
///     .arg("FOO")
///     .note(DiagId::note_previous_definition, prev_span)
///     .emit(&engine);
/// ```
///
/// The caret goes at `span.lo`. The primary span is *not* underlined;
/// pass ranges to underline with [`span`](Self::span) or
/// [`range`](Self::range) (clang's `^~~~` is `.range(span)` on the
/// primary span).
#[derive(Clone, Debug)]
#[must_use = "a diagnostic does nothing until it is emitted"]
pub struct Diag {
    pub(crate) id: Option<DiagId>,
    pub(crate) text: Option<String>,
    pub(crate) span: Span,
    pub(crate) args: Vec<String>,
    pub(crate) labels: Vec<Label>,
    pub(crate) fixits: Vec<FixIt>,
    pub(crate) notes: Vec<Diag>,
    pub(crate) allow_duplicate: bool,
}

impl Diag {
    /// Starts a diagnostic with id `id` at `span`.
    pub fn new(id: DiagId, span: Span) -> Diag {
        Diag {
            id: Some(id),
            text: None,
            span,
            args: Vec::new(),
            labels: Vec::new(),
            fixits: Vec::new(),
            notes: Vec::new(),
            allow_duplicate: false,
        }
    }

    /// Appends the next template argument.
    pub fn arg(mut self, a: impl Display) -> Diag {
        self.args.push(a.to_string());
        self
    }

    /// The diagnostic's id (`None` for a free-text note).
    pub fn id(&self) -> Option<DiagId> {
        self.id
    }

    /// Adds a secondary span with a label (the label shows in JSON only).
    pub fn span(mut self, span: Span, label: impl Into<String>) -> Diag {
        self.labels.push(Label { span, text: label.into() });
        self
    }

    /// Adds an unlabeled range to underline.
    pub fn range(self, span: Span) -> Diag {
        self.span(span, String::new())
    }

    /// Adds a note at `span` with an argument-less note id or free text.
    pub fn note(mut self, msg: impl Into<NoteMsg>, span: Span) -> Diag {
        let n = match msg.into() {
            NoteMsg::Id(id) => Diag::new(id, span),
            NoteMsg::Text(t) => Diag {
                id: None,
                text: Some(t),
                ..Diag::new(DiagId::note_previous_definition, span)
            },
        };
        self.notes.push(n);
        self
    }

    /// Adds a fully built note (for note ids with arguments, ranges or
    /// fix-its). The note's own notes are appended after it.
    pub fn with_note(mut self, mut note: Diag) -> Diag {
        let nested = std::mem::take(&mut note.notes);
        self.notes.push(note);
        self.notes.extend(nested);
        self
    }

    /// Inserts `notes` before the diagnostic's own notes (the template
    /// instantiation context precedes them, spec/sema-templates.md §10.2).
    pub fn prepend_notes(&mut self, notes: Vec<Diag>) {
        if notes.is_empty() {
            return;
        }
        let own = std::mem::replace(&mut self.notes, notes);
        self.notes.extend(own);
    }

    /// Whether the diagnostic has notes attached.
    pub fn has_notes(&self) -> bool {
        !self.notes.is_empty()
    }

    /// Adds a fix-it hint.
    pub fn fixit(mut self, f: FixIt) -> Diag {
        self.fixits.push(f);
        self
    }

    /// Exempts this diagnostic from `(id, span)` deduplication.
    pub fn allow_duplicate(mut self) -> Diag {
        self.allow_duplicate = true;
        self
    }

    /// Hands the diagnostic to `engine`, which applies policy and sinks it.
    pub fn emit(self, engine: &DiagEngine<'_>) {
        engine.emit(self);
    }

    /// The rendered message.
    pub(crate) fn message(&self) -> String {
        match (&self.text, self.id) {
            (Some(t), _) => t.clone(),
            (None, Some(id)) => {
                debug_assert_eq!(
                    self.args.len(),
                    id.arg_count(),
                    "{} takes {} argument(s)",
                    id.name(),
                    id.arg_count()
                );
                template::render(id.template(), &self.args)
            }
            (None, None) => String::new(),
        }
    }
}

/// An emitted diagnostic, as sinks receive it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// The id; `None` for free-text notes.
    pub id: Option<DiagId>,
    /// The level after policy.
    pub level: Level,
    /// The rendered message (without the option suffix).
    pub message: String,
    /// The primary span; the caret is at `span.lo`. May be invalid.
    pub span: Span,
    /// Secondary spans.
    pub labels: Vec<Label>,
    /// Fix-it hints.
    pub fixits: Vec<FixIt>,
    /// What clang prints between `[` `]` after the message
    /// (`-Wmacro-redefined`, `-Werror,-W#warnings`, `-ferror-limit=`), if
    /// anything.
    pub option: Option<String>,
    /// Attached notes, in order (not including macro-expansion notes,
    /// which renderers derive from the spans).
    pub notes: Vec<Diagnostic>,
}

impl Diagnostic {
    /// The warning-group name shown for this diagnostic, if any.
    pub fn group(&self) -> Option<&'static str> {
        self.id.and_then(DiagId::display_group).map(crate::Group::name)
    }

    pub(crate) fn from_note(n: &Diag) -> Diagnostic {
        Diagnostic {
            id: n.id,
            level: Level::Note,
            message: n.message(),
            span: n.span,
            labels: n.labels.clone(),
            fixits: n.fixits.clone(),
            option: None,
            notes: Vec::new(),
        }
    }
}
