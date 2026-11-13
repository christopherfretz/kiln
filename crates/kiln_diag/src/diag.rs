//! The `Diag` builder and the emitted `Diagnostic` record (`spec/diag.md`
//! sections "Building" and "Diagnostic records").

use std::fmt::Display;

use kiln_snap::Snap;
use kiln_source::{Loc, Span};

use crate::engine::DiagEngine;
use crate::ids::DiagId;
use crate::template;

/// The level a diagnostic is rendered with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Snap)]
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
#[derive(Clone, Debug, PartialEq, Eq, Snap)]
pub struct Label {
    /// The highlighted range.
    pub span: Span,
    /// Free text describing the range (JSON only; may be empty).
    pub text: String,
}

/// A suggested edit: replace `span` by `text`. Stored and rendered (the
/// insertion line under the caret, JSON `fixits`); never applied.
#[derive(Clone, Debug, PartialEq, Eq, Snap)]
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
#[derive(Clone, Debug, PartialEq, Eq, Snap)]
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
#[derive(Clone, Debug, Snap)]
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
    /// Not a duplicate of the same `(id, span)` reported with context 0
    /// (spec/diag.md §7.4): a check clang re-runs on each instantiation.
    pub(crate) per_instantiation: bool,
    /// Part of the deduplication key: the context the diagnostic was
    /// emitted in (sema's instantiation stack; 0 = none). The same
    /// `(id, span)` under two contexts prints twice.
    pub(crate) context: u64,
    /// A note printed (and counted) as an error: clang's evaluation notes
    /// that carry an `err_` id (spec/diag.md §3.3).
    pub(crate) error_note: bool,
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
            per_instantiation: false,
            context: 0,
            error_note: false,
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

    /// The `i`th template argument, as rendered (clang's
    /// `PartialDiagnostic::getStringArg`).
    pub fn arg_at(&self, i: usize) -> Option<&str> {
        self.args.get(i).map(String::as_str)
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

    /// A free-text note at `span`, standing alone (a context note built
    /// from evaluator text, for [`Diag::prepend_notes`]).
    pub fn text_note(text: impl Into<String>, span: Span) -> Diag {
        Diag {
            id: None,
            text: Some(text.into()),
            ..Diag::new(DiagId::note_previous_definition, span)
        }
    }

    /// Adds a free-text note at `span` printed and counted as an error
    /// (clang emits an evaluation note with an `err_` id at its own
    /// severity, e.g. a `-1` `__builtin_shufflevector` index in a
    /// constant expression).
    pub fn error_note(self, text: impl Into<String>, span: Span) -> Diag {
        let mut d = self.note(NoteMsg::Text(text.into()), span);
        if let Some(n) = d.notes.last_mut() {
            n.error_note = true;
        }
        d
    }

    /// Adds a fully built note (for note ids with arguments, ranges or
    /// fix-its). The note's own notes are appended after it.
    pub fn with_note(mut self, mut note: Diag) -> Diag {
        let nested = std::mem::take(&mut note.notes);
        self.notes.push(note);
        self.notes.extend(nested);
        self
    }

    /// Inserts `notes` (context notes such as clang's `in implicit ...
    /// first required here`) before this diagnostic's other notes, as
    /// clang prints the code-synthesis context right after the
    /// diagnostic itself.
    pub fn with_context_notes(mut self, notes: Vec<Diag>) -> Diag {
        let rest = std::mem::replace(&mut self.notes, notes);
        self.notes.extend(rest);
        self
    }

    /// Where the caret goes.
    pub const fn loc(&self) -> Loc {
        self.span.lo
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

    /// Moves the notes located at `from` to `to` (a held-back
    /// diagnostic's context notes, reported for a later use).
    pub fn relocate_notes(&mut self, from: Loc, to: Loc) {
        for n in &mut self.notes {
            if n.span.lo == from {
                n.span = Span::empty(to);
            }
        }
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

    /// Sets the emission context that joins `(id, span)` in the
    /// deduplication key (spec/diag.md §7.4): sema passes a hash of the
    /// instantiation stack, so an error inside a template prints once per
    /// instantiation rather than once per source location.
    pub fn context(mut self, key: u64) -> Diag {
        self.context = key;
        self
    }

    /// Exempts this diagnostic from `(id, span, context)` deduplication.
    pub fn allow_duplicate(mut self) -> Diag {
        self.allow_duplicate = true;
        self
    }

    /// Keeps `(id, span, context)` deduplication but drops the rule that
    /// an instantiation's diagnostic repeats one from the template
    /// definition (spec/diag.md §7.4): for checks clang re-runs on the
    /// rebuilt node of every instantiation, e.g. `-Wreturn-stack-address`.
    pub fn per_instantiation(mut self) -> Diag {
        self.per_instantiation = true;
        self
    }

    /// Hands the diagnostic to `engine`, which applies policy and sinks it.
    pub fn emit(self, engine: &DiagEngine<'_>) {
        engine.emit(self);
    }

    /// Replaces the ids of this diagnostic and its notes by the ones that
    /// report them at clang compatibility level `level`
    /// ([`DiagId::for_compat`]).
    pub(crate) fn set_compat(&mut self, level: u32) {
        if level >= 22 {
            return;
        }
        if let Some(id) = self.id {
            self.id = Some(id.for_compat(level));
        }
        for n in &mut self.notes {
            n.set_compat(level);
        }
    }

    /// The rendered message at clang compatibility level `level`
    /// ([`DiagId::for_compat`]; what the engine prints for this
    /// diagnostic, for a message embedded in another one).
    pub fn message_at(&self, level: u32) -> String {
        match (&self.text, self.id) {
            (None, Some(id)) if level < 22 => {
                template::render(id.for_compat(level).template(), &self.args)
            }
            _ => self.message(),
        }
    }

    /// The rendered message.
    pub fn message(&self) -> String {
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
#[derive(Clone, Debug, PartialEq, Eq, Snap)]
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
            level: if n.error_note { Level::Error } else { Level::Note },
            message: n.message(),
            span: n.span,
            labels: n.labels.clone(),
            fixits: n.fixits.clone(),
            option: None,
            notes: Vec::new(),
        }
    }
}
