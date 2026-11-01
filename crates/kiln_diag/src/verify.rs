//! `-verify` (spec/diag.md §11): clang's `VerifyDiagnosticConsumer`.
//! Comments are scanned for `expected-*` directives as the preprocessor
//! lexes them ([`Verifier::handle_comment`]); at the end the buffered
//! diagnostics are matched against the directives ([`Verifier::check`])
//! and the differences become a [`VerifyReport`], printed in clang's
//! words. The grammar, the matching rules and the report format follow
//! clang 19's `VerifyDiagnosticConsumer.cpp` line by line, quirks
//! included.

mod regex;

use std::collections::BTreeMap;

use kiln_source::{FileId, Loc, SourceManager, Span};

use crate::{Diag, DiagEngine, DiagId, Diagnostic, Level};
pub use regex::{Regex, escape as regex_escape};

/// The four diagnostic levels `-verify` checks, in checking order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VerifyLevel {
    /// `expected-error` (fatal errors included).
    Error,
    /// `expected-warning`.
    Warning,
    /// `expected-remark`.
    Remark,
    /// `expected-note`.
    Note,
}

impl VerifyLevel {
    /// Checking (and report) order.
    pub const ALL: [VerifyLevel; 4] =
        [VerifyLevel::Error, VerifyLevel::Warning, VerifyLevel::Remark, VerifyLevel::Note];

    /// `error`, `warning`, `remark` or `note`.
    pub fn name(self) -> &'static str {
        match self {
            VerifyLevel::Error => "error",
            VerifyLevel::Warning => "warning",
            VerifyLevel::Remark => "remark",
            VerifyLevel::Note => "note",
        }
    }

    /// The level named `s` (a `-verify-ignore-unexpected=` value).
    pub fn from_name(s: &str) -> Option<VerifyLevel> {
        VerifyLevel::ALL.into_iter().find(|l| l.name() == s)
    }

    fn of(level: Level) -> VerifyLevel {
        match level {
            Level::Error | Level::Fatal => VerifyLevel::Error,
            Level::Warning => VerifyLevel::Warning,
            Level::Remark => VerifyLevel::Remark,
            Level::Note => VerifyLevel::Note,
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// The `-verify` options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifyOptions {
    /// The directive prefixes (`-verify=<p>,...`; `expected` for plain
    /// `-verify`), sorted and deduplicated, never empty.
    pub prefixes: Vec<String>,
    /// `-verify-ignore-unexpected[=<levels>]`: unexpected diagnostics of
    /// these levels are not reported, indexed by [`VerifyLevel`].
    pub ignore_unexpected: [bool; 4],
}

impl Default for VerifyOptions {
    fn default() -> Self {
        VerifyOptions { prefixes: vec!["expected".to_owned()], ignore_unexpected: [false; 4] }
    }
}

impl VerifyOptions {
    /// Whether `p` is a valid `-verify=` prefix: a letter, then letters,
    /// digits, `-` and `_` (clang `checkVerifyPrefixes`).
    pub fn is_valid_prefix(p: &str) -> bool {
        let mut b = p.bytes();
        b.next().is_some_and(|c| c.is_ascii_alphabetic())
            && b.all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    }

    /// The prefix named in reports: the alphabetically first.
    pub fn first_prefix(&self) -> &str {
        self.prefixes.first().map_or("expected", String::as_str)
    }
}

/// Resolves the file of `@<file>:<line>` like a quoted `#include` written
/// at the directive's location.
pub type FileLookup<'a> = Box<dyn FnMut(&SourceManager, Loc, &str) -> Option<FileId> + 'a>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Status {
    NoDirectives,
    ExpectedNoDiagnostics,
    OtherDirectives,
}

#[derive(Debug)]
enum Matcher {
    Substring,
    Regex(Regex),
    /// An invalid regex (already diagnosed): matches nothing.
    Invalid,
}

/// One `expected-*` directive.
#[derive(Debug)]
struct Directive {
    directive_loc: Loc,
    /// Where the diagnostic is expected; invalid for `@*` / `@*:*`.
    diag_loc: Loc,
    any_file_and_line: bool,
    any_line: bool,
    text: String,
    matcher: Matcher,
    min: u32,
    max: u32,
}

impl Directive {
    fn matches(&self, s: &str) -> bool {
        match &self.matcher {
            Matcher::Substring => s.contains(self.text.as_str()),
            Matcher::Regex(r) => r.is_match(s),
            Matcher::Invalid => false,
        }
    }
}

/// A directive waiting for its location (clang `UnattachedDirective`).
#[derive(Clone, Debug)]
struct Unattached {
    level: VerifyLevel,
    regex: bool,
    directive_loc: Loc,
    content_loc: Loc,
    text: String,
    min: u32,
    max: u32,
}

#[derive(Clone, Copy, Debug)]
struct Marker {
    def: Loc,
    redef: Loc,
    use_: Loc,
}

/// clang's `Directive::MaxCount`.
const MAX_COUNT: u32 = u32::MAX;

/// At most this many missing occurrences are listed per level.
const MAX_LISTED: usize = 10_000;

/// The `-verify` state for one translation unit.
pub struct Verifier<'a> {
    sm: &'a SourceManager,
    opts: VerifyOptions,
    main: Option<FileId>,
    lookup: FileLookup<'a>,
    status: Status,
    lists: [Vec<Directive>; 4],
    markers: BTreeMap<String, Marker>,
    deferred: BTreeMap<String, Vec<Unattached>>,
}

impl std::fmt::Debug for Verifier<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Verifier").field("opts", &self.opts).finish_non_exhaustive()
    }
}

/// The differences found by [`Verifier::check`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VerifyReport {
    /// The report's diagnostics, in order: `(id, args)`, all located
    /// nowhere.
    pub entries: Vec<(DiagId, Vec<String>)>,
    /// clang's error count: one per listed diagnostic line, plus one for
    /// "no expected directives found".
    pub problems: u32,
}

impl VerifyReport {
    /// Emits the report's diagnostics to `engine`.
    pub fn emit(&self, engine: &DiagEngine<'_>) {
        for (id, args) in &self.entries {
            let mut d = Diag::new(*id, Span::INVALID).allow_duplicate();
            for a in args {
                d = d.arg(a);
            }
            d.emit(engine);
        }
    }

    /// The closing `N errors generated.` line, if anything was wrong.
    pub fn summary(&self) -> Option<String> {
        match self.problems {
            0 => None,
            1 => Some("1 error generated.".to_owned()),
            n => Some(format!("{n} errors generated.")),
        }
    }
}

impl<'a> Verifier<'a> {
    /// A verifier for the translation unit whose main file is `main`.
    pub fn new(
        sm: &'a SourceManager,
        opts: VerifyOptions,
        main: Option<FileId>,
        lookup: FileLookup<'a>,
    ) -> Self {
        Verifier {
            sm,
            opts,
            main,
            lookup,
            status: Status::NoDirectives,
            lists: Default::default(),
            markers: BTreeMap::new(),
            deferred: BTreeMap::new(),
        }
    }

    /// clang `HandleComment`: scans the comment at `comment` (its text
    /// including `//` or `/* */`) for markers and directives. Errors in
    /// directives are emitted to `engine` (they are then unexpected
    /// diagnostics like any other).
    pub fn handle_comment(&mut self, comment: Span, engine: &DiagEngine<'_>) {
        let Some((file, lo)) = self.sm.offset_of(comment.lo) else { return };
        let Some((_, hi)) = self.sm.offset_of(comment.hi) else { return };
        let text = self.sm.text(file);
        let Some(c) = text.get(lo as usize..hi as usize) else { return };
        if c.is_empty() {
            return;
        }
        if !c.contains('\\') {
            self.parse_directives(c, comment.lo, engine);
            return;
        }
        let folded = fold_line_splices(c);
        if !folded.is_empty() {
            self.parse_directives(&folded, comment.lo, engine);
        }
    }

    /// clang `ParseDirective`; returns whether a directive was found.
    fn parse_directives(&mut self, s: &str, pos: Loc, engine: &DiagEngine<'_>) -> bool {
        let at = |ph: &Helper<'_>| Span::empty(pos.with_offset(ph.c as u32));
        // Markers first.
        let mut ph = Helper::new(s);
        while !ph.done() {
            if !ph.search(b"#", true, false) {
                break;
            }
            ph.c = ph.p;
            if !ph.next_marker() {
                ph.next(b"#");
                ph.advance();
                continue;
            }
            ph.advance();
            let name = ph.matched().to_owned();
            self.add_marker(name, pos, engine);
        }

        let mut found = false;
        let mut ph = Helper::new(s);
        while !ph.done() {
            let ok = if self.opts.prefixes.len() == 1 {
                let p = self.opts.prefixes[0].clone();
                ph.search(p.as_bytes(), true, true)
            } else {
                ph.search(b"", true, true)
            };
            if !ok {
                break;
            }
            let mut dtoken = ph.matched();
            ph.advance();

            let mut regex = false;
            let mut kind = "string";
            if let Some(t) = dtoken.strip_suffix("-re") {
                dtoken = t;
                regex = true;
                kind = "regex";
            }
            let mut level = None;
            let rest = if let Some(t) = dtoken.strip_suffix("-error") {
                level = Some(VerifyLevel::Error);
                t
            } else if let Some(t) = dtoken.strip_suffix("-warning") {
                level = Some(VerifyLevel::Warning);
                t
            } else if let Some(t) = dtoken.strip_suffix("-remark") {
                level = Some(VerifyLevel::Remark);
                t
            } else if let Some(t) = dtoken.strip_suffix("-note") {
                level = Some(VerifyLevel::Note);
                t
            } else if let Some(t) = dtoken.strip_suffix("-no-diagnostics") {
                if regex {
                    continue;
                }
                t
            } else {
                continue;
            };
            if self.opts.prefixes.binary_search_by(|p| p.as_str().cmp(rest)).is_err() {
                continue;
            }
            let prefix = self.opts.first_prefix().to_owned();
            let Some(level) = level else {
                // `-no-diagnostics`.
                if self.status == Status::OtherDirectives {
                    Diag::new(DiagId::diag_verify_invalid_no_diags, Span::empty(pos))
                        .arg(&prefix)
                        .arg(1)
                        .allow_duplicate()
                        .emit(engine);
                } else {
                    self.status = Status::ExpectedNoDiagnostics;
                }
                continue;
            };
            if self.status == Status::ExpectedNoDiagnostics {
                Diag::new(DiagId::diag_verify_invalid_no_diags, Span::empty(pos))
                    .arg(&prefix)
                    .arg(0)
                    .allow_duplicate()
                    .emit(engine);
                continue;
            }
            self.status = Status::OtherDirectives;

            // `@` location.
            let mut expected = Loc::INVALID;
            let mut marker = None;
            let mut any_file_and_line = false;
            let mut any_line = false;
            if !ph.next(b"@") {
                expected = pos;
            } else {
                ph.advance();
                let plus = ph.next(b"+");
                if plus || ph.next(b"-") {
                    ph.advance();
                    if let Some(cur) = self.sm.line_col(pos).map(|lc| lc.line)
                        && let Some(n) = ph.next_number()
                        && (plus || n < cur)
                    {
                        let line = if plus { cur.saturating_add(n) } else { cur - n };
                        expected = self.translate_line(pos, line);
                    }
                } else if let Some(n) = ph.next_number() {
                    if n > 0 {
                        expected = self.translate_line(pos, n);
                    }
                } else if ph.next_marker() {
                    marker = Some(ph.matched().to_owned());
                } else if ph.search(b":", false, false) {
                    let filename = s[ph.c..ph.p].to_owned();
                    ph.advance();
                    if filename == "*" {
                        any_file_and_line = true;
                        if !ph.next(b"*") {
                            Diag::new(DiagId::diag_verify_missing_line, at(&ph))
                                .arg("'*'")
                                .allow_duplicate()
                                .emit(engine);
                            continue;
                        }
                        any_line = true;
                        expected = Loc::INVALID;
                    } else {
                        let Some(fid) = (self.lookup)(self.sm, pos, &filename) else {
                            Diag::new(DiagId::diag_verify_missing_file, at(&ph))
                                .arg(&filename)
                                .arg(kind)
                                .allow_duplicate()
                                .emit(engine);
                            continue;
                        };
                        let base = self.sm.loc(fid, 0);
                        if let Some(n) = ph.next_number().filter(|&n| n > 0) {
                            expected = self.translate_line(base, n);
                        } else if ph.next(b"*") {
                            any_line = true;
                            expected = self.translate_line(base, 1);
                        }
                    }
                } else if ph.next(b"*") {
                    any_line = true;
                    expected = Loc::INVALID;
                }
                if !expected.is_valid() && !any_line && marker.is_none() {
                    Diag::new(DiagId::diag_verify_missing_line, at(&ph))
                        .arg(kind)
                        .allow_duplicate()
                        .emit(engine);
                    continue;
                }
                ph.advance();
            }

            ph.skip_whitespace();
            // Count: `N`, `N+`, `N-M`, `+`.
            let (mut min, mut max) = (1, 1);
            if let Some(n) = ph.next_number() {
                min = n;
                ph.advance();
                if ph.next(b"+") {
                    max = MAX_COUNT;
                    ph.advance();
                } else if ph.next(b"-") {
                    ph.advance();
                    match ph.next_number() {
                        Some(m) if m >= min => max = m,
                        _ => {
                            Diag::new(DiagId::diag_verify_invalid_range, at(&ph))
                                .arg(kind)
                                .allow_duplicate()
                                .emit(engine);
                            continue;
                        }
                    }
                    ph.advance();
                } else {
                    max = min;
                }
            } else if ph.next(b"+") {
                max = MAX_COUNT;
                ph.advance();
            }

            ph.skip_whitespace();
            if !ph.next(b"{{") {
                Diag::new(DiagId::diag_verify_missing_start, at(&ph))
                    .arg(kind)
                    .allow_duplicate()
                    .emit(engine);
                continue;
            }
            let mut close = String::from("}}");
            let delim_begin = ph.c;
            ph.advance();
            while !regex && ph.next(b"{") {
                close.push('}');
                ph.advance();
            }
            let content_begin = ph.c;
            let open = s[delim_begin..content_begin].to_owned();
            if !ph.search_closing_brace(open.as_bytes(), close.as_bytes()) {
                Diag::new(DiagId::diag_verify_missing_end, at(&ph))
                    .arg(kind)
                    .arg(&close)
                    .allow_duplicate()
                    .emit(engine);
                continue;
            }
            let content_end = ph.p;
            ph.advance();
            let content_loc = pos.with_offset(content_begin as u32);

            // `\n` becomes a newline. clang drops the text after the last
            // `\n`; when there is none the text is the whole content.
            let content = &s[content_begin..content_end];
            let mut text = String::new();
            let mut cpos = 0;
            while let Some(f) = content[cpos..].find("\\n") {
                text.push_str(&content[cpos..cpos + f]);
                text.push('\n');
                cpos += f + 2;
            }
            if text.is_empty() {
                text = content.to_owned();
            }
            if regex && !text.contains("{{") {
                Diag::new(DiagId::diag_verify_missing_regex, Span::empty(content_loc))
                    .arg(&text)
                    .allow_duplicate()
                    .emit(engine);
                return false;
            }
            let ud = Unattached { level, regex, directive_loc: pos, content_loc, text, min, max };
            match marker {
                None => self.attach(&ud, expected, any_file_and_line, any_line, engine),
                Some(m) => self.add_directive_at_marker(m, ud, engine),
            }
            found = true;
        }
        found
    }

    /// clang `SourceManager::translateLineCol(FileID of at, line, 1)`:
    /// the start of `line` in the inclusion holding `at`, clamped to the
    /// last byte of the file.
    fn translate_line(&self, at: Loc, line: u32) -> Loc {
        let Some((file, off)) = self.sm.offset_of(at) else { return Loc::INVALID };
        let base = at.to_raw() - off;
        let text = self.sm.text(file);
        let target = if line > self.sm.line_count(file) {
            (text.len() as u32).saturating_sub(1)
        } else {
            match self.sm.line_text(file, line) {
                Some(l) => (l.as_ptr() as usize - text.as_ptr() as usize) as u32,
                None => (text.len() as u32).saturating_sub(1),
            }
        };
        Loc::from_raw(base + target)
    }

    fn attach(
        &mut self,
        ud: &Unattached,
        diag_loc: Loc,
        any_file_and_line: bool,
        any_line: bool,
        engine: &DiagEngine<'_>,
    ) {
        let matcher = if ud.regex {
            match Regex::new(&regex_source(&ud.text)) {
                Ok(r) => Matcher::Regex(r),
                Err(e) => {
                    Diag::new(DiagId::diag_verify_invalid_content, Span::empty(ud.content_loc))
                        .arg("regex")
                        .arg(e)
                        .allow_duplicate()
                        .emit(engine);
                    Matcher::Invalid
                }
            }
        } else {
            Matcher::Substring
        };
        self.lists[ud.level.index()].push(Directive {
            directive_loc: ud.directive_loc,
            diag_loc,
            any_file_and_line,
            any_line,
            text: ud.text.clone(),
            matcher,
            min: ud.min,
            max: ud.max,
        });
    }

    /// clang `MarkerTracker::addMarker`.
    fn add_marker(&mut self, name: String, pos: Loc, engine: &DiagEngine<'_>) {
        if let Some(m) = self.markers.get_mut(&name) {
            m.redef = pos;
            return;
        }
        let mut m = Marker { def: pos, redef: Loc::INVALID, use_: Loc::INVALID };
        for ud in self.deferred.remove(&name).unwrap_or_default() {
            if !m.use_.is_valid() {
                m.use_ = ud.directive_loc;
            }
            self.attach(&ud, pos, false, false, engine);
        }
        self.markers.insert(name, m);
    }

    fn add_directive_at_marker(&mut self, name: String, ud: Unattached, engine: &DiagEngine<'_>) {
        if let Some(m) = self.markers.get_mut(&name) {
            if !m.use_.is_valid() {
                m.use_ = ud.directive_loc;
            }
            let def = m.def;
            self.attach(&ud, def, false, false, engine);
            return;
        }
        self.deferred.entry(name).or_default().push(ud);
    }
}

impl Verifier<'_> {
    /// clang `MarkerTracker::finalize`: markers that are redefined and
    /// used, and directives naming markers never defined. Call once all
    /// comments are handled, before [`check`](Self::check), with the
    /// engine whose diagnostics `check` will see.
    pub fn finish(&mut self, engine: &DiagEngine<'_>) {
        for (name, m) in &self.markers {
            if m.redef.is_valid() && m.use_.is_valid() {
                let here = |at: Loc| {
                    Diag::new(DiagId::diag_verify_ambiguous_marker_here, Span::empty(at)).arg(name)
                };
                Diag::new(DiagId::diag_verify_ambiguous_marker, Span::empty(m.use_))
                    .arg(name)
                    .with_note(here(m.def))
                    .with_note(here(m.redef))
                    .allow_duplicate()
                    .emit(engine);
            }
        }
        for (name, uds) in std::mem::take(&mut self.deferred) {
            if let Some(ud) = uds.first() {
                Diag::new(DiagId::diag_verify_no_such_marker, Span::empty(ud.directive_loc))
                    .arg(&name)
                    .allow_duplicate()
                    .emit(engine);
            }
        }
    }

    /// clang `CheckDiagnostics`: matches `diags` (everything the
    /// translation unit reported, notes included, in emission order)
    /// against the directives.
    pub fn check(&self, diags: &[Diagnostic]) -> VerifyReport {
        let mut report = VerifyReport::default();
        let prefix = self.opts.first_prefix();
        if self.status == Status::NoDirectives {
            report.entries.push((DiagId::diag_verify_no_directives, vec![prefix.to_owned()]));
            report.problems += 1;
        }
        let mut seen: [Vec<(Loc, &str)>; 4] = Default::default();
        fn flatten<'d>(d: &'d Diagnostic, seen: &mut [Vec<(Loc, &'d str)>; 4]) {
            seen[VerifyLevel::of(d.level).index()].push((d.span.lo, d.message.as_str()));
            for n in &d.notes {
                flatten(n, seen);
            }
        }
        for d in diags {
            flatten(d, &mut seen);
        }
        for level in VerifyLevel::ALL {
            let i = level.index();
            let kind = format!("{prefix}-{}", level.name());
            let (expected, unexpected) = self.check_list(&self.lists[i], &seen[i]);
            if !expected.is_empty() {
                report.problems += expected.len() as u32;
                let body: String = expected.iter().map(|d| self.expected_line(d)).collect();
                report.entries.push((
                    DiagId::diag_verify_inconsistent_diags,
                    vec![kind.clone(), "0".to_owned(), body],
                ));
            }
            if !self.opts.ignore_unexpected[i] && !unexpected.is_empty() {
                report.problems += unexpected.len() as u32;
                let body: String =
                    unexpected.iter().map(|&(loc, msg)| self.seen_line(loc, msg)).collect();
                report.entries.push((
                    DiagId::diag_verify_inconsistent_diags,
                    vec![kind, "1".to_owned(), body],
                ));
            }
        }
        report
    }

    /// clang `CheckLists`: the directives (once per missing occurrence)
    /// and the diagnostics left unmatched.
    fn check_list<'d, 's>(
        &self,
        directives: &'d [Directive],
        seen: &[(Loc, &'s str)],
    ) -> (Vec<&'d Directive>, Vec<(Loc, &'s str)>) {
        let mut left_only = Vec::new();
        let mut right: Vec<(Loc, &'s str)> = seen.to_vec();
        for d in directives {
            let line1 = self.presumed_line(d.diag_loc);
            let mut i = 0;
            while i < d.max {
                let found = right.iter().position(|&(loc, msg)| {
                    (d.any_line || self.presumed_line(loc) == line1)
                        && (!d.diag_loc.is_valid()
                            || d.any_file_and_line
                            || self.is_from_same_file(d.diag_loc, loc))
                        && d.matches(msg)
                });
                match found {
                    Some(k) => {
                        right.remove(k);
                    }
                    None => {
                        // The cap departs from clang only for absurd
                        // counts (`expected-error 4000000000 {{x}}`),
                        // which would otherwise exhaust memory.
                        if i >= d.min || left_only.len() >= MAX_LISTED {
                            break;
                        }
                        left_only.push(d);
                    }
                }
                i += 1;
            }
        }
        (left_only, right)
    }

    fn presumed_line(&self, loc: Loc) -> u32 {
        self.sm.presumed_line_col(loc).map_or(0, |p| p.line)
    }

    /// The buffer of a file location, if it is a real file (clang: has a
    /// `FileEntry`; built-in buffers such as `<built-in>` have none).
    fn file_entry(&self, loc: Loc) -> Option<FileId> {
        let (fid, _) = self.sm.offset_of(loc)?;
        (!self.sm.file_name(fid).starts_with('<')).then_some(fid)
    }

    /// clang `IsFromSameFile`.
    fn is_from_same_file(&self, directive: Loc, mut diag: Loc) -> bool {
        while self.sm.is_macro_loc(diag) {
            let up = self.sm.immediate_macro_caller_loc(diag);
            if up == diag {
                break;
            }
            diag = up;
        }
        let dfile = self.sm.offset_of(diag).map(|(f, _)| f);
        let xfile = self.sm.offset_of(directive).map(|(f, _)| f);
        if dfile.is_some() && dfile == xfile {
            return true;
        }
        let dentry = self.file_entry(diag);
        if dentry.is_none() && xfile.is_some() && xfile == self.main {
            return true;
        }
        dentry.is_none() && self.file_entry(directive).is_none()
    }

    fn file_name(&self, loc: Loc) -> &str {
        self.sm.offset_of(loc).map_or("", |(f, _)| self.sm.file_name(f))
    }

    /// clang `PrintExpected`, one entry.
    fn expected_line(&self, d: &Directive) -> String {
        let mut out = String::from("\n  File ");
        if !d.diag_loc.is_valid() || d.any_file_and_line {
            out.push('*');
        } else {
            out.push_str(self.file_name(d.diag_loc));
        }
        if d.any_line {
            out.push_str(" Line *");
        } else {
            out.push_str(&format!(" Line {}", self.presumed_line(d.diag_loc)));
        }
        if d.directive_loc != d.diag_loc {
            out.push_str(&format!(
                " (directive at {}:{})",
                self.file_name(d.directive_loc),
                self.presumed_line(d.directive_loc)
            ));
        }
        out.push_str(": ");
        out.push_str(&d.text);
        out
    }

    /// clang `PrintUnexpected`, one entry.
    fn seen_line(&self, loc: Loc, msg: &str) -> String {
        let mut out = String::new();
        if !loc.is_valid() {
            out.push_str("\n  (frontend)");
        } else {
            out.push_str("\n ");
            if !self.sm.is_macro_loc(loc)
                && let Some(f) = self.file_entry(loc)
            {
                out.push_str(" File ");
                out.push_str(self.sm.file_name(f));
            }
            out.push_str(&format!(" Line {}", self.presumed_line(loc)));
        }
        out.push_str(": ");
        out.push_str(msg);
        out
    }
}

/// clang folds `\` line splices out of a comment before parsing it.
fn fold_line_splices(c: &str) -> String {
    let b = c.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut last = 0;
    while let Some(rel) = b[last..].iter().position(|&x| x == b'\\') {
        let loc = last + rel;
        out.extend_from_slice(&b[last..loc]);
        last = loc + 1;
        match b.get(last) {
            Some(&e @ (b'\n' | b'\r')) => {
                last += 1;
                if let Some(&e2 @ (b'\n' | b'\r')) = b.get(last)
                    && e2 != e
                {
                    last += 1;
                }
            }
            _ => out.push(b'\\'),
        }
    }
    out.extend_from_slice(&b[last.min(b.len())..]);
    String::from_utf8_lossy(&out).into_owned()
}

/// clang `Directive::create`: the literal parts escaped, each `{{...}}`
/// a parenthesized regex.
fn regex_source(text: &str) -> String {
    let mut out = String::new();
    let mut s = text;
    while !s.is_empty() {
        if let Some(rest) = s.strip_prefix("{{") {
            let n = rest.find("}}").unwrap_or(rest.len());
            out.push('(');
            out.push_str(&rest[..n]);
            out.push(')');
            s = rest.get(n + 2..).unwrap_or("");
        } else {
            let n = s.find("{{").unwrap_or(s.len());
            out.push_str(&regex::escape(&s[..n]));
            s = &s[n..];
        }
    }
    out
}

/// clang's `ParseHelper`, over bytes.
struct Helper<'s> {
    s: &'s [u8],
    text: &'s str,
    c: usize,
    p: usize,
    pend: usize,
}

impl<'s> Helper<'s> {
    fn new(text: &'s str) -> Self {
        Helper { s: text.as_bytes(), text, c: 0, p: 0, pend: 0 }
    }

    fn next(&mut self, lit: &[u8]) -> bool {
        self.p = self.c;
        self.pend = self.c + lit.len();
        self.pend <= self.s.len() && &self.s[self.p..self.pend] == lit
    }

    fn next_number(&mut self) -> Option<u32> {
        self.p = self.c;
        self.pend = self.p;
        let mut n: u32 = 0;
        while let Some(d) = self.s.get(self.pend).filter(|d| d.is_ascii_digit()) {
            n = n.wrapping_mul(10).wrapping_add(u32::from(d - b'0'));
            self.pend += 1;
        }
        (self.pend != self.c).then_some(n)
    }

    fn next_marker(&mut self) -> bool {
        self.p = self.c;
        if self.s.get(self.p) != Some(&b'#') {
            return false;
        }
        self.pend = self.p + 1;
        while self.s.get(self.pend).is_some_and(|&b| is_directive_char(b)) {
            self.pend += 1;
        }
        self.pend > self.p + 1
    }

    fn search(&mut self, lit: &[u8], start_of_word: bool, finish_token: bool) -> bool {
        loop {
            if lit.is_empty() {
                self.p = self.c;
                while self.p < self.s.len() && !self.s[self.p].is_ascii_alphabetic() {
                    self.p += 1;
                }
                self.pend = self.p + 1;
            } else {
                self.p = find(&self.s[self.c.min(self.s.len())..], lit)
                    .map_or(self.s.len(), |i| self.c + i);
                self.pend = self.p + lit.len();
            }
            if self.p >= self.s.len() {
                return false;
            }
            let p = self.p;
            let word_start = p == 0
                || is_whitespace(self.s[p - 1])
                || (p > 1 && matches!(self.s[p - 1], b'/' | b'*') && self.s[p - 2] == b'/');
            if start_of_word && !word_start {
                if self.advance() {
                    continue;
                }
                return false;
            }
            if finish_token {
                while self.s.get(self.pend).is_some_and(|&b| is_directive_char(b)) {
                    self.pend += 1;
                }
                while self.pend > self.p + 1
                    && (self.s[self.pend - 1].is_ascii_digit() || self.s[self.pend - 1] == b'-')
                {
                    self.pend -= 1;
                }
            }
            return true;
        }
    }

    fn search_closing_brace(&mut self, open: &[u8], close: &[u8]) -> bool {
        let mut depth = 1;
        self.p = self.c;
        while self.p < self.s.len() {
            let rest = &self.s[self.p..];
            if rest.starts_with(open) {
                depth += 1;
                self.p += open.len();
            } else if rest.starts_with(close) {
                depth -= 1;
                if depth == 0 {
                    self.pend = self.p + close.len();
                    return true;
                }
                self.p += close.len();
            } else {
                self.p += 1;
            }
        }
        false
    }

    fn advance(&mut self) -> bool {
        self.c = self.pend;
        self.c < self.s.len()
    }

    fn matched(&self) -> &'s str {
        self.text.get(self.p..self.pend.min(self.s.len())).unwrap_or("")
    }

    fn skip_whitespace(&mut self) {
        while self.s.get(self.c).is_some_and(|&b| is_whitespace(b)) {
            self.c += 1;
        }
    }

    fn done(&self) -> bool {
        self.c >= self.s.len()
    }
}

fn is_directive_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
}

/// clang `isWhitespace`: space, `\t`, `\n`, `\v`, `\f`, `\r`.
fn is_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}
