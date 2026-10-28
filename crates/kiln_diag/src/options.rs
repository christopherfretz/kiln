//! Command-line options that control diagnostics (`spec/diag.md` section
//! "Options").

use crate::ids::DiagId;

/// Output format of the default sink.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Format {
    /// clang's text format (the default).
    #[default]
    Text,
    /// One JSON object per diagnostic per line
    /// (`-fdiagnostics-format=json`).
    Json,
}

/// A diagnostic produced while parsing options; [`DiagEngine::new`]
/// emits these first, in order.
///
/// [`DiagEngine::new`]: crate::DiagEngine::new
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionDiag {
    /// The diagnostic.
    pub id: DiagId,
    /// Its arguments.
    pub args: Vec<String>,
}

/// Diagnostic options, filled by [`DiagOptions::parse_arg`] in
/// command-line order. Defaults match the clang driver.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct DiagOptions {
    /// `-W` options in order, without the `-W` (`"error"`,
    /// `"no-unused-macros"`, `"error=foo"`, `"everything"`, ...).
    pub warnings: Vec<String>,
    /// `-R` options in order, without the `-R`.
    pub remarks: Vec<String>,
    /// `-w`: ignore all warnings (not positional).
    pub ignore_warnings: bool,
    /// `-pedantic`: extension diagnostics become warnings.
    pub pedantic: bool,
    /// `-pedantic-errors`: extension diagnostics become errors.
    pub pedantic_errors: bool,
    /// `-ferror-limit=N`; 0 is unlimited. Default 19 (the clang driver's).
    pub error_limit: u32,
    /// `-fmacro-backtrace-limit=N`; 0 is unlimited. Default 6.
    pub macro_backtrace_limit: u32,
    /// `-ftabstop=N`, 1..=100. Default 8.
    pub tabstop: u32,
    /// Maximum number of source lines in one snippet. Default 16.
    pub snippet_line_limit: u32,
    /// `-fcolor-diagnostics`. Default off.
    pub show_colors: bool,
    /// `-fcaret-diagnostics` (source snippets). Default on.
    pub show_carets: bool,
    /// `-fdiagnostics-show-line-numbers` (the `  12 | ` gutter). Default on.
    pub show_line_numbers: bool,
    /// `-fdiagnostics-show-option` (` [-Wgroup]`). Default on.
    pub show_option: bool,
    /// `-fshow-column`. Default on.
    pub show_column: bool,
    /// `-fshow-source-location`. Default on.
    pub show_location: bool,
    /// `-fdiagnostics-fixit-info` (fix-it insertion lines). Default on.
    pub show_fixits: bool,
    /// `-fdiagnostics-format=`.
    pub format: Format,
    /// Problems found while parsing the options above.
    pub pending: Vec<OptionDiag>,
}

/// Default and maximum `-ftabstop` (clang's `DefaultTabStop`/`MaxTabStop`).
pub const DEFAULT_TABSTOP: u32 = 8;
const MAX_TABSTOP: u32 = 100;

impl Default for DiagOptions {
    fn default() -> Self {
        DiagOptions {
            warnings: Vec::new(),
            remarks: Vec::new(),
            ignore_warnings: false,
            pedantic: false,
            pedantic_errors: false,
            error_limit: 19,
            macro_backtrace_limit: 6,
            tabstop: DEFAULT_TABSTOP,
            snippet_line_limit: 16,
            show_colors: false,
            show_carets: true,
            show_line_numbers: true,
            show_option: true,
            show_column: true,
            show_location: true,
            show_fixits: true,
            format: Format::Text,
            pending: Vec::new(),
        }
    }
}

impl DiagOptions {
    /// Options from a list of arguments; arguments that are not
    /// diagnostic options are ignored.
    pub fn from_args<I, S>(args: I) -> DiagOptions
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut o = DiagOptions::default();
        for a in args {
            o.parse_arg(a.as_ref());
        }
        o
    }

    /// Applies one command-line argument. Returns true if it is a
    /// diagnostic option (consumed), false if it is not. Bad values are
    /// recorded in [`pending`](Self::pending).
    ///
    /// Accepted: `-w`, `-W<x>` (except `-Wl,`/`-Wa,`/`-Wp,`), `-WX` (alias
    /// of `-Werror`), `-R<x>` (except `-Rpass*`), `-pedantic`,
    /// `--pedantic`, `-pedantic-errors`, `-ferror-limit=N`,
    /// `-fmacro-backtrace-limit=N`, `-ftabstop=N`,
    /// `-f[no-]color-diagnostics`, `-f[no-]diagnostics-color`,
    /// `-fdiagnostics-color=always|never|auto`, `-f[no-]caret-diagnostics`,
    /// `-f[no-]diagnostics-show-line-numbers`,
    /// `-f[no-]diagnostics-show-option`, `-f[no-]show-column`,
    /// `-f[no-]show-source-location`, `-f[no-]diagnostics-fixit-info`,
    /// `-fdiagnostics-format=clang|json`.
    pub fn parse_arg(&mut self, arg: &str) -> bool {
        match arg {
            "-w" => self.ignore_warnings = true,
            "-WX" => self.warnings.push("error".to_string()),
            "-pedantic" | "--pedantic" => self.pedantic = true,
            "-pedantic-errors" | "--pedantic-errors" => self.pedantic_errors = true,
            "-fcolor-diagnostics" | "-fdiagnostics-color" => self.show_colors = true,
            "-fno-color-diagnostics" | "-fno-diagnostics-color" => self.show_colors = false,
            "-fcaret-diagnostics" => self.show_carets = true,
            "-fno-caret-diagnostics" => self.show_carets = false,
            "-fdiagnostics-show-line-numbers" => self.show_line_numbers = true,
            "-fno-diagnostics-show-line-numbers" => self.show_line_numbers = false,
            "-fdiagnostics-show-option" => self.show_option = true,
            "-fno-diagnostics-show-option" => self.show_option = false,
            "-fshow-column" => self.show_column = true,
            "-fno-show-column" => self.show_column = false,
            "-fshow-source-location" => self.show_location = true,
            "-fno-show-source-location" => self.show_location = false,
            "-fdiagnostics-fixit-info" => self.show_fixits = true,
            "-fno-diagnostics-fixit-info" => self.show_fixits = false,
            _ => return self.parse_valued(arg),
        }
        true
    }

    fn parse_valued(&mut self, arg: &str) -> bool {
        if let Some(w) = arg.strip_prefix("-W") {
            if w.is_empty() || w.starts_with("l,") || w.starts_with("a,") || w.starts_with("p,") {
                return false;
            }
            self.warnings.push(w.to_string());
        } else if let Some(r) = arg.strip_prefix("-R") {
            if r.is_empty() || r.starts_with("pass") {
                return false;
            }
            self.remarks.push(r.to_string());
        } else if let Some(v) = arg.strip_prefix("-ferror-limit=") {
            if let Some(n) = self.int_value("-ferror-limit", v) {
                self.error_limit = n;
            }
        } else if let Some(v) = arg.strip_prefix("-fmacro-backtrace-limit=") {
            if let Some(n) = self.int_value("-fmacro-backtrace-limit", v) {
                self.macro_backtrace_limit = n;
            }
        } else if let Some(v) = arg.strip_prefix("-ftabstop=") {
            if let Some(n) = self.int_value("-ftabstop", v) {
                if n == 0 || n > MAX_TABSTOP {
                    self.pending.push(OptionDiag {
                        id: DiagId::drv_ignoring_tabstop,
                        args: vec![v.to_string(), DEFAULT_TABSTOP.to_string()],
                    });
                    self.tabstop = DEFAULT_TABSTOP;
                } else {
                    self.tabstop = n;
                }
            }
        } else if let Some(v) = arg.strip_prefix("-fdiagnostics-color=") {
            match v {
                "always" => self.show_colors = true,
                // kiln does not probe the terminal: `auto` means off.
                "never" | "auto" => self.show_colors = false,
                _ => self.invalid_value("-fdiagnostics-color=", v),
            }
        } else if let Some(v) = arg.strip_prefix("-fdiagnostics-format=") {
            match v {
                "clang" => self.format = Format::Text,
                "json" => self.format = Format::Json,
                _ => self.invalid_value("-fdiagnostics-format", v),
            }
        } else {
            return false;
        }
        true
    }

    fn int_value(&mut self, name: &str, v: &str) -> Option<u32> {
        let parsed = v.parse::<u32>().ok();
        if parsed.is_none() {
            self.pending.push(OptionDiag {
                id: DiagId::drv_invalid_int_value,
                args: vec![format!("{name} {v}"), v.to_string()],
            });
        }
        parsed
    }

    fn invalid_value(&mut self, name: &str, v: &str) {
        let shown = if name.ends_with('=') { format!("{name}{v}") } else { format!("{name} {v}") };
        self.pending
            .push(OptionDiag { id: DiagId::drv_invalid_value, args: vec![shown, v.to_string()] });
    }
}
