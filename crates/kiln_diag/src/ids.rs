//! The diagnostic table: every [`DiagId`] with its kind, warning group,
//! argument count and message template (`spec/diag.md` sections 2.1, 2.4).
//!
//! The rows live in one file per area under `src/ids/` (`ids/pp.rs`,
//! `ids/sema_a_b.rs`, ...), sorted by name; `build.rs` joins them into the
//! `diag_id_table!` macro, which passes every row to `diag_ids!` below.
//! Each row reads
//!
//! ```text
//! name = Kind [Group] (nargs) "template";
//! ```
//!
//! where `[Group]` is optional and names a [`Group`] variant, `nargs` is the
//! number of arguments the emitter passes with [`Diag::arg`](crate::Diag::arg),
//! and the template uses `{0}`, `{1}`, ... (see [`crate::template`]). A
//! compile-time assertion checks every template against its `nargs`.
//! Wording follows clang's wherever clang has the diagnostic; sema passes
//! type arguments pre-quoted, with clang's `(aka '...')` suffix when sugar
//! hides the canonical type (`Sema::ty_arg`).
//!
//! To add an id, add its row to the area file its name selects (spec/diag.md
//! §2.4), or to the tail block at the end of this file and run `cargo xtask
//! diag-split`, which moves it there; `cargo xtask ci` checks the layout.

use crate::groups::Group;
use crate::policy::Severity;
use crate::template;

/// The kind of a diagnostic: its class (error, warning, extension, remark,
/// note) and its default severity, mirroring clang's `TableGen` classes
/// (`Error`, `Warning`, `DefaultIgnore`, `DefaultError`, `ExtWarn`, ...).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DiagKind {
    /// A hard error (clang `Error`). Not controllable by `-W` options.
    Error,
    /// A fatal error (clang `DefaultFatal`): stops all later diagnostics.
    Fatal,
    /// A warning, on by default.
    Warning,
    /// A warning, off by default (clang `DefaultIgnore`).
    WarningOff,
    /// A warning that is an error by default (clang `DefaultError`); `-w`
    /// does not silence it.
    WarningError,
    /// A warning that `-Werror` does not promote (clang
    /// `DefaultWarnNoWerror`), e.g. `#pragma message`.
    WarningNoWerror,
    /// An extension diagnostic, off unless `-pedantic` (clang `Extension`).
    Extension,
    /// An extension diagnostic, on by default (clang `ExtWarn`).
    ExtWarn,
    /// A remark, off unless `-R<group>` (clang `Remark`).
    Remark,
    /// A note; only ever attached to another diagnostic.
    Note,
}

/// Diagnostic class, as clang's `CLASS_*`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DiagClass {
    /// Hard and fatal errors.
    Error,
    /// Warnings of every default severity.
    Warning,
    /// Extension diagnostics (`-pedantic` affects them).
    Extension,
    /// Remarks.
    Remark,
    /// Notes.
    Note,
}

impl DiagKind {
    /// The class of diagnostics of this kind.
    pub const fn class(self) -> DiagClass {
        match self {
            DiagKind::Error | DiagKind::Fatal => DiagClass::Error,
            DiagKind::Warning
            | DiagKind::WarningOff
            | DiagKind::WarningError
            | DiagKind::WarningNoWerror => DiagClass::Warning,
            DiagKind::Extension | DiagKind::ExtWarn => DiagClass::Extension,
            DiagKind::Remark => DiagClass::Remark,
            DiagKind::Note => DiagClass::Note,
        }
    }

    /// Severity before any option is applied. Notes report `Ignored`
    /// (they follow their parent).
    pub const fn default_severity(self) -> Severity {
        match self {
            DiagKind::Error | DiagKind::WarningError => Severity::Error,
            DiagKind::Fatal => Severity::Fatal,
            DiagKind::Warning | DiagKind::WarningNoWerror | DiagKind::ExtWarn => Severity::Warning,
            DiagKind::WarningOff | DiagKind::Extension | DiagKind::Remark | DiagKind::Note => {
                Severity::Ignored
            }
        }
    }
}

macro_rules! diag_ids {
    ($( $name:ident = $kind:ident $([$group:ident])? ($n:literal) $tmpl:literal; )*) => {
        /// A stable diagnostic identifier. See the table in `src/ids/`.
        ///
        /// Variants are `snake_case` so that the name printed in JSON output
        /// and used in test expectations is the variant name verbatim.
        #[allow(non_camel_case_types)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(u16)]
        pub enum DiagId {
            $(
                #[doc = concat!("`", $tmpl, "`")]
                $name,
            )*
        }

        impl DiagId {
            /// Every id, in table order (`ALL[id as usize] == id`): the area
            /// files in file-name order, each sorted by name, then the tail.
            pub const ALL: &'static [DiagId] = &[$(DiagId::$name),*];

            /// The id's name (the variant name).
            pub const fn name(self) -> &'static str {
                match self { $(DiagId::$name => stringify!($name),)* }
            }

            /// The id's kind (class and default severity).
            pub const fn kind(self) -> DiagKind {
                match self { $(DiagId::$name => DiagKind::$kind,)* }
            }

            /// The warning group the id belongs to, if any.
            pub const fn group(self) -> Option<Group> {
                match self { $(DiagId::$name => diag_ids!(@group $($group)?),)* }
            }

            /// Number of arguments the message template takes.
            pub const fn arg_count(self) -> usize {
                match self { $(DiagId::$name => $n,)* }
            }

            /// The message template (`{0}` placeholders, `{0:a|b}` selects).
            pub const fn template(self) -> &'static str {
                match self { $(DiagId::$name => $tmpl,)* }
            }
        }

        // Every template uses exactly placeholders 0..nargs (spec/diag.md
        // "Ids": the static arg-count check).
        $(
            const _: () = assert!(
                template::check($tmpl, $n),
                concat!("template of `", stringify!($name), "` does not match its argument count"),
            );
        )*
    };
    (@group $g:ident) => { Some(Group::$g) };
    (@group) => { None };
}

// The table: `src/ids/*.rs`, joined by `build.rs` (spec/diag.md §2.4).
include!(concat!(env!("OUT_DIR"), "/diag_id_table.rs"));

impl DiagId {
    /// Number of ids in the table.
    pub const COUNT: usize = DiagId::ALL.len();

    /// The id's class.
    pub const fn class(self) -> DiagClass {
        self.kind().class()
    }

    /// Severity before options are applied.
    pub const fn default_severity(self) -> Severity {
        self.kind().default_severity()
    }

    /// True for warnings and extensions: the ids `-W` options control
    /// (clang's `isBuiltinWarningOrExtension`).
    pub const fn is_warning_or_extension(self) -> bool {
        matches!(self.class(), DiagClass::Warning | DiagClass::Extension)
    }

    /// True if the id is an error unless options say otherwise (clang's
    /// `isDefaultMappingAsError`).
    pub const fn is_default_error(self) -> bool {
        matches!(self.default_severity(), Severity::Error | Severity::Fatal)
    }

    /// True if the id is reported even when located in a system header
    /// (clang's `ShowInSystemHeader`): hard errors, and the few warnings
    /// clang marks so (checked against clang 19 with `-isystem`).
    pub const fn shows_in_system_header(self) -> bool {
        matches!(self.class(), DiagClass::Error)
            || matches!(
                self,
                DiagId::pp_warning_directive
                    | DiagId::oven_enum_constexpr_conversion
                    | DiagId::sema_diagnose_if_succeeded_warning
            )
    }

    /// The group whose name is shown in `[-W...]` and in JSON output:
    /// [`group`](Self::group), or `pedantic` for an extension diagnostic
    /// without a group (clang puts those in `-Wpedantic`).
    pub fn display_group(self) -> Option<Group> {
        self.group().or((self.class() == DiagClass::Extension).then_some(Group::Pedantic))
    }

    /// Looks an id up by name.
    pub fn from_name(name: &str) -> Option<DiagId> {
        DiagId::ALL.iter().copied().find(|id| id.name() == name)
    }

    /// The id that reports this diagnostic at clang compatibility level
    /// `level` (`-fclang-compat=`; spec/diag.md §2.5): below 22, an id
    /// whose wording clang 22 changed is replaced by its `<name>_19` row
    /// (clang 19's wording, same kind, group and arguments). Every other id,
    /// and every id at level 22, is itself.
    #[must_use]
    pub fn for_compat(self, level: u32) -> DiagId {
        static MAP: std::sync::OnceLock<Vec<DiagId>> = std::sync::OnceLock::new();
        if level >= 22 {
            return self;
        }
        let map = MAP.get_or_init(|| {
            let mut map = DiagId::ALL.to_vec();
            for &old in DiagId::ALL {
                if let Some(base) = old.name().strip_suffix("_19").and_then(DiagId::from_name) {
                    map[base as usize] = old;
                }
            }
            map
        });
        map[self as usize]
    }
}

// The tail block (spec/diag.md §2.4): rows here compile, but `cargo xtask
// diag-split --check` (run by `cargo xtask ci`) fails until `cargo xtask
// diag-split` has moved them into their area file under `src/ids/`.
diag_id_table! {
}
