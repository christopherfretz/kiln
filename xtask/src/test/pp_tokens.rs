//! Token-wise comparison of preprocessed output (`spec/harness.md` "pp
//! comparison"): both outputs are re-lexed with `kiln_lex` and compared as
//! pp-token sequences, ignoring whitespace. Punctuators compare by kind,
//! everything else by category and spelling. In line-marker mode
//! (`-E` without `-P`) the `# N "file" flags` lines are parsed into
//! include enter/leave events and a presumed (file, line) for every token,
//! and those are compared too.

use std::fmt::Write as _;
use std::ops::Range;

use kiln_lex::{Interner, LexOptions, Lexer, Punct, Std, TokFlags, TokKind, Token};
use kiln_source::SourceManager;

/// How many tokens a mismatch report lists from each side.
const SHOW: usize = 10;
/// Context tokens shown before and after a mismatch.
const CONTEXT: usize = 3;
/// A resync is searched for this many tokens ahead on each side.
const RESYNC_WINDOW: usize = 64;
/// A resync needs this many equal tokens in a row.
const RESYNC_RUN: usize = 8;

/// The lexing rules for re-lexing output of a test compiled with `-std=<std>`.
/// C has no lexer mode yet; C output is re-lexed with the C++98 rules (no
/// raw strings, ud-suffixes, `<::` exception, digit separators), the closest.
pub fn lex_std(std: &str) -> Std {
    match std {
        "c++98" => Std::Cxx98,
        "c++03" => Std::Cxx03,
        "c++11" => Std::Cxx11,
        "c++14" => Std::Cxx14,
        "c++17" => Std::Cxx17,
        "c++20" => Std::Cxx20,
        "c++23" => Std::Cxx23,
        s if s.starts_with('c') && !s.starts_with("c++") => Std::Cxx98,
        _ => Std::Cxx26,
    }
}

/// Broad token category; non-punctuator tokens are equal when the category
/// and the spelling are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cat {
    Ident,
    Number,
    Char,
    Str,
    RawStr,
    HeaderName,
    Other,
    Unknown,
}

impl Cat {
    fn name(self) -> &'static str {
        match self {
            Cat::Ident => "identifier",
            Cat::Number => "pp-number",
            Cat::Char => "character-literal",
            Cat::Str => "string-literal",
            Cat::RawStr => "raw-string-literal",
            Cat::HeaderName => "header-name",
            Cat::Other => "other",
            Cat::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum What {
    Punct(Punct),
    /// `cleaned` indexes [`Lexed::cleaned`] when the token's spelling is not
    /// its output text (it contained a line splice).
    Spelled {
        cat: Cat,
        cleaned: Option<u32>,
    },
    /// An include enter (flag 1) or leave (flag 2) line marker.
    Marker,
}

/// One item of the compared sequence.
#[derive(Clone, Copy, Debug)]
struct Item {
    what: What,
    /// Byte range in the output: the token, or the whole marker line.
    lo: u32,
    hi: u32,
    /// Line-marker mode: presumed file (index into [`Lexed::files`],
    /// `u32::MAX` before the first marker) and line.
    file: u32,
    line: u32,
    /// Line-marker mode: the marker's flags (bit `n` = flag `n`); for tokens,
    /// the system-header flags 3 and 4 in effect.
    flags: u8,
}

/// A re-lexed preprocessed output.
#[derive(Debug)]
pub struct Lexed {
    text: String,
    items: Vec<Item>,
    cleaned: Vec<String>,
    files: Vec<String>,
}

const NO_FILE: u32 = u32::MAX;
const SYSTEM_FLAGS: u8 = (1 << 3) | (1 << 4);

/// A parsed line marker: `# N "file" flags` or `#line N "file"`.
struct Marker {
    line: u32,
    file: Option<String>,
    flags: u8,
}

impl Lexed {
    /// Re-lexes `text`. With `markers`, line-marker lines are parsed (and
    /// removed from the token sequence) and every token gets a presumed
    /// location.
    pub fn new(text: &str, std: Std, markers: bool) -> Self {
        let sm = SourceManager::new();
        let mut out = Lexed {
            text: text.to_owned(),
            items: Vec::new(),
            cleaned: Vec::new(),
            files: Vec::new(),
        };
        let Ok(file) = sm.add_virtual("<pp-output>", text.to_owned()) else {
            // Too large for a source buffer (4 GiB): compare as one opaque item.
            out.items.push(Item {
                what: What::Spelled { cat: Cat::Unknown, cleaned: None },
                lo: 0,
                hi: text.len() as u32,
                file: NO_FILE,
                line: 0,
                flags: 0,
            });
            return out;
        };
        let interner = Interner::new();
        let opts = LexOptions::new(std);
        let mut lx = Lexer::new(&sm, &interner, file, sm.loc(file, 0), opts);
        let mut state = Presumed::default();
        let mut line: Vec<(Token, Range<u32>)> = Vec::new();
        let mut newlines = Newlines::default();
        loop {
            let t = lx.next();
            let range = sm
                .offset_of(t.span.lo)
                .zip(sm.offset_of(t.span.hi))
                .map_or(0..0, |((_, lo), (_, hi))| lo..hi);
            let eof = t.kind == TokKind::Eof;
            if (eof || t.has(TokFlags::START_OF_LINE)) && !line.is_empty() {
                out.flush_line(&line, markers, &mut state, &mut newlines, opts);
                line.clear();
            }
            if eof {
                break;
            }
            line.push((t, range));
        }
        out
    }

    fn flush_line(
        &mut self,
        line: &[(Token, Range<u32>)],
        markers: bool,
        state: &mut Presumed,
        newlines: &mut Newlines,
        opts: LexOptions,
    ) {
        let phys = newlines.line_at(&self.text, line[0].1.start);
        if markers && let Some(m) = self.parse_marker(line, opts) {
            let file = match m.file {
                Some(f) => self.intern_file(&f),
                None => state.file,
            };
            let lo = line[0].1.start;
            let hi = line.last().map_or(lo, |(_, r)| r.end);
            let entering = m.flags & (1 << 1) != 0;
            let leaving = m.flags & (1 << 2) != 0;
            // Enter/leave events are compared, except those into or out of
            // the predefines pseudo-files (`<built-in>`, `<command line>`).
            // A leave marker names the file returned to; the file left is
            // the current one.
            let left = leaving.then(|| {
                state.stack.pop();
                state.file
            });
            let pseudo = |f: u32| self.files.get(f as usize).is_some_and(|n| n.starts_with('<'));
            if (entering || leaving) && !pseudo(file) && !left.is_some_and(pseudo) {
                self.items.push(Item {
                    what: What::Marker,
                    lo,
                    hi,
                    file,
                    line: m.line,
                    flags: m.flags,
                });
            }
            if entering && state.file != NO_FILE {
                state.stack.push(state.file);
            }
            *state = Presumed {
                file,
                base_line: m.line,
                base_phys: phys + 1,
                flags: m.flags & SYSTEM_FLAGS,
                stack: std::mem::take(&mut state.stack),
            };
            return;
        }
        for (t, r) in line {
            let what = match t.kind {
                TokKind::Punct(p) => What::Punct(p),
                k => {
                    let cat = match k {
                        TokKind::Identifier(_) => Cat::Ident,
                        TokKind::PpNumber => Cat::Number,
                        TokKind::CharLit(_) => Cat::Char,
                        TokKind::StringLit(_) => Cat::Str,
                        TokKind::RawStringLit(_) => Cat::RawStr,
                        TokKind::HeaderName => Cat::HeaderName,
                        TokKind::Other(_) => Cat::Other,
                        _ => Cat::Unknown,
                    };
                    let cleaned = t.has(TokFlags::NEEDS_CLEANING).then(|| {
                        let raw = &self.text[r.start as usize..r.end as usize];
                        self.cleaned.push(kiln_lex::clean(raw, t.kind, opts.trigraphs));
                        (self.cleaned.len() - 1) as u32
                    });
                    What::Spelled { cat, cleaned }
                }
            };
            let (file, line) = if markers {
                let p = newlines.line_at(&self.text, r.start);
                (state.file, state.presumed_line(p))
            } else {
                (NO_FILE, 0)
            };
            self.items.push(Item { what, lo: r.start, hi: r.end, file, line, flags: state.flags });
        }
    }

    /// Recognizes `# N ["file" [flags...]]` and `#line N ["file"]`.
    fn parse_marker(&self, line: &[(Token, Range<u32>)], opts: LexOptions) -> Option<Marker> {
        let text = |i: usize| -> Option<String> {
            let (t, r) = line.get(i)?;
            let raw = &self.text[r.start as usize..r.end as usize];
            Some(if t.has(TokFlags::NEEDS_CLEANING) {
                kiln_lex::clean(raw, t.kind, opts.trigraphs)
            } else {
                raw.to_owned()
            })
        };
        let (first, _) = line.first()?;
        if first.kind != TokKind::Punct(Punct::Hash) || first.has(TokFlags::DIGRAPH) {
            return None;
        }
        let mut i = 1;
        if matches!(line.get(1), Some((t, _)) if matches!(t.kind, TokKind::Identifier(_)))
            && text(1)? == "line"
        {
            i = 2;
        }
        let (num, _) = line.get(i)?;
        if num.kind != TokKind::PpNumber {
            return None;
        }
        let number: u32 = text(i)?.parse().ok()?;
        i += 1;
        let file = match line.get(i) {
            None => return Some(Marker { line: number, file: None, flags: 0 }),
            Some((t, _)) if t.kind == TokKind::StringLit(kiln_lex::Encoding::Ordinary) => {
                Some(normalize_path(&text(i)?))
            }
            Some(_) => return None,
        };
        let mut flags = 0u8;
        for (k, (t, _)) in line.iter().enumerate().skip(i + 1) {
            if t.kind != TokKind::PpNumber {
                return None;
            }
            match text(k)?.as_str() {
                d @ ("1" | "2" | "3" | "4") => flags |= 1 << (d.as_bytes()[0] - b'0'),
                _ => return None,
            }
        }
        Some(Marker { line: number, file, flags })
    }

    fn intern_file(&mut self, name: &str) -> u32 {
        if let Some(i) = self.files.iter().position(|f| f == name) {
            return i as u32;
        }
        self.files.push(name.to_owned());
        (self.files.len() - 1) as u32
    }

    /// Number of compared items (tokens plus enter/leave markers).
    pub fn len(&self) -> usize {
        self.items.len()
    }

    fn spelling(&self, it: &Item) -> &str {
        match it.what {
            What::Spelled { cleaned: Some(i), .. } => &self.cleaned[i as usize],
            _ => &self.text[it.lo as usize..it.hi as usize],
        }
    }

    fn file_name(&self, it: &Item) -> &str {
        self.files.get(it.file as usize).map_or("", String::as_str)
    }

    /// 1-based physical line and column (in characters) of byte `off`.
    fn line_col(&self, off: u32) -> (usize, usize) {
        let before = &self.text[..off as usize];
        let line = before.bytes().filter(|&b| b == b'\n').count() + 1;
        let start = before.rfind('\n').map_or(0, |i| i + 1);
        (line, before[start..].chars().count() + 1)
    }

    fn line_text(&self, off: u32) -> &str {
        let off = off as usize;
        let start = self.text[..off].rfind('\n').map_or(0, |i| i + 1);
        let end = self.text[off..].find('\n').map_or(self.text.len(), |i| off + i);
        &self.text[start..end]
    }

    /// How an item is shown in a mismatch report.
    fn describe(&self, it: &Item, markers: bool) -> String {
        let (l, c) = self.line_col(it.lo);
        let mut s = format!("{l}:{c}");
        if markers && it.what != What::Marker && it.file != NO_FILE {
            let _ = write!(s, " [{}:{}{}]", self.file_name(it), it.line, sys_suffix(it.flags));
        }
        match it.what {
            What::Marker => {
                let _ = write!(s, "  # {} {}{}", it.line, self.file_name(it), flag_list(it.flags));
                s.push_str("  (line marker)");
            }
            What::Punct(p) => {
                let _ = write!(s, "  {}", p.as_str());
            }
            What::Spelled { cat, .. } => {
                let sp = self.spelling(it);
                let shown: String = sp.chars().take(120).collect();
                let more = if shown.len() < sp.len() { "..." } else { "" };
                let _ = write!(s, "  {}{more}  ({})", shown.escape_debug(), cat.name());
            }
        }
        s
    }
}

fn sys_suffix(flags: u8) -> &'static str {
    match flags & SYSTEM_FLAGS {
        0 => "",
        x if x == 1 << 3 => " sys",
        _ => " sys extern-C",
    }
}

fn flag_list(flags: u8) -> String {
    let mut s = String::new();
    for n in (1..=4).filter(|n| flags & (1 << n) != 0) {
        let _ = write!(s, " {n}");
    }
    s
}

/// Lexical path normalization of a marker file name (the spelling of a
/// string literal, quotes included): `//` and `/./` collapse, `dir/..`
/// cancels. clang prints search-path spellings such as
/// `/usr/lib/llvm-19/bin/../include/c++/v1/vector`; a compiler that
/// prints the normalized path is not wrong.
fn normalize_path(lit: &str) -> String {
    let Some(inner) = lit.strip_prefix('"').and_then(|s| s.strip_suffix('"')) else {
        return lit.to_owned();
    };
    if inner.starts_with('<') {
        return inner.to_owned();
    }
    let absolute = inner.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for p in inner.split('/') {
        match p {
            "" | "." => {}
            ".." if parts.last().is_some_and(|l| *l != "..") => {
                parts.pop();
            }
            ".." if absolute => {}
            p => parts.push(p),
        }
    }
    let joined = parts.join("/");
    if absolute { format!("/{joined}") } else { joined }
}

struct Presumed {
    file: u32,
    base_line: u32,
    base_phys: usize,
    flags: u8,
    /// Files entered with flag 1 and not yet left.
    stack: Vec<u32>,
}

impl Default for Presumed {
    fn default() -> Self {
        Presumed { file: NO_FILE, base_line: 1, base_phys: 1, flags: 0, stack: Vec::new() }
    }
}

impl Presumed {
    fn presumed_line(&self, phys: usize) -> u32 {
        (self.base_line as usize + phys.saturating_sub(self.base_phys)) as u32
    }
}

/// Incremental physical line numbers for increasing byte offsets.
#[derive(Default)]
struct Newlines {
    pos: usize,
    line: usize,
}

impl Newlines {
    fn line_at(&mut self, text: &str, off: u32) -> usize {
        let off = off as usize;
        if off < self.pos {
            self.pos = 0;
            self.line = 0;
        }
        self.line += text[self.pos..off].matches('\n').count();
        self.pos = off;
        self.line + 1
    }
}

/// Are items `i` of `a` and `j` of `b` the same?
#[allow(clippy::many_single_char_names)]
fn same(a: &Lexed, i: usize, b: &Lexed, j: usize, markers: bool) -> bool {
    let (x, y) = (&a.items[i], &b.items[j]);
    let kind_eq = match (x.what, y.what) {
        (What::Punct(p), What::Punct(q)) => p == q,
        (What::Spelled { cat: c, .. }, What::Spelled { cat: d, .. }) => {
            c == d && a.spelling(x) == b.spelling(y)
        }
        (What::Marker, What::Marker) => true,
        _ => false,
    };
    kind_eq
        && (!markers
            || (x.line == y.line && x.flags == y.flags && a.file_name(x) == b.file_name(y)))
}

/// Compares two preprocessed outputs token-wise. `None` if they are
/// equivalent, else a report of the first difference. `labels` name the two
/// sides (`("expected", "actual")`, `("oracle", "actual")`).
pub fn diff(
    expected: &str,
    actual: &str,
    std: Std,
    markers: bool,
    labels: (&str, &str),
) -> Option<String> {
    let a = Lexed::new(expected, std, markers);
    let b = Lexed::new(actual, std, markers);
    diff_lexed(&a, &b, markers, labels)
}

#[allow(clippy::many_single_char_names)]
fn diff_lexed(a: &Lexed, b: &Lexed, markers: bool, labels: (&str, &str)) -> Option<String> {
    let k = (0..a.len().min(b.len())).find(|&k| !same(a, k, b, k, markers)).or_else(|| {
        // One is a prefix of the other.
        (a.len() != b.len()).then_some(a.len().min(b.len()))
    })?;
    let (la, lb) = labels;
    let mut out = format!(
        "token streams differ at token {} ({la}: {} tokens, {lb}: {} tokens){}\n",
        k + 1,
        a.len(),
        b.len(),
        if markers { "; [file:line] is the presumed location from line markers" } else { "" }
    );
    // The smallest skip on each side after which RESYNC_RUN items agree.
    let resync = (1..=2 * RESYNC_WINDOW).find_map(|total| {
        (0..=total).find_map(|da| {
            let db = total - da;
            if da > RESYNC_WINDOW || db > RESYNC_WINDOW {
                return None;
            }
            let (i, j) = (k + da, k + db);
            let run = RESYNC_RUN.min(a.len().saturating_sub(i)).min(b.len().saturating_sub(j));
            let ends_together = a.len() - i.min(a.len()) == b.len() - j.min(b.len());
            let agree = (0..run).all(|n| same(a, i + n, b, j + n, markers));
            (agree && (run == RESYNC_RUN || ends_together) && i <= a.len() && j <= b.len())
                .then_some((da, db))
        })
    });
    for n in k.saturating_sub(CONTEXT)..k {
        let _ = writeln!(out, " {}", a.describe(&a.items[n], markers));
    }
    let (da, db) = resync.unwrap_or((SHOW, SHOW));
    for n in k..(k + da).min(a.len()) {
        let _ = writeln!(out, "-{}", a.describe(&a.items[n], markers));
    }
    // A side's end is shown when the hunk runs into it: one output is a
    // truncation of the other, or the listing (no resync) is cut short.
    let ends = |len: usize, d: usize, other: usize| k + d >= len && (resync.is_none() || d < other);
    if ends(a.len(), da, db) {
        let _ = writeln!(out, "-<end of {la}>");
    }
    for n in k..(k + db).min(b.len()) {
        let _ = writeln!(out, "+{}", b.describe(&b.items[n], markers));
    }
    if ends(b.len(), db, da) {
        let _ = writeln!(out, "+<end of {lb}>");
    }
    match resync {
        Some((da, _)) => {
            for n in (k + da)..(k + da + CONTEXT).min(a.len()) {
                let _ = writeln!(out, " {}", a.describe(&a.items[n], markers));
            }
            let rest = (a.len() - (k + da).min(a.len())).saturating_sub(CONTEXT);
            if rest > 0 {
                let _ = writeln!(out, " ... ({rest} more {la} tokens not compared in this report)");
            }
        }
        None => {
            let _ = writeln!(out, "(no resync within {RESYNC_WINDOW} tokens; showing {SHOW})");
        }
    }
    if let Some(x) = a.items.get(k) {
        let _ = writeln!(out, "{la} line {}: {}", a.line_col(x.lo).0, clip(a.line_text(x.lo)));
    }
    if let Some(y) = b.items.get(k) {
        let _ = writeln!(out, "{lb} line {}: {}", b.line_col(y.lo).0, clip(b.line_text(y.lo)));
    }
    Some(out)
}

fn clip(s: &str) -> String {
    const MAX: usize = 200;
    if s.chars().count() <= MAX {
        s.to_owned()
    } else {
        let mut t: String = s.chars().take(MAX).collect();
        t.push_str("...");
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eq(a: &str, b: &str) -> bool {
        diff(a, b, Std::Cxx26, false, ("expected", "actual")).is_none()
    }

    fn eq_markers(a: &str, b: &str) -> bool {
        diff(a, b, Std::Cxx26, true, ("expected", "actual")).is_none()
    }

    // [cpp.output]-style spacing never matters, token boundaries do.
    #[test]
    fn whitespace_is_ignored_token_boundaries_are_not() {
        assert!(eq("a+b", "a + b"));
        assert!(eq("int x ;\n\n\n f ( 1 ) ;", "int x;f(1);"));
        assert!(eq("", "\n\n  \n"));
        assert!(!eq("a b", "ab"));
        assert!(!eq("+ +", "++"));
        assert!(!eq("- -x", "--x"));
        assert!(!eq("x", "x y"));
        // A splice inside a token is not part of its spelling.
        assert!(eq("ab\\\ncd", "abcd"));
    }

    // [lex.digraph]: punctuators compare by kind, so `<:` equals `[`.
    #[test]
    fn punctuators_compare_by_kind() {
        assert!(eq("a<:1:>", "a[1]"));
        assert!(eq("%:define", "#define"));
        // [lex.pptoken]/3.2: `<::` not followed by `:` or `>` is `<` `::`.
        assert!(eq("<::", "< ::"));
        assert!(!eq("<:::", "< :: :"));
    }

    // Literals and identifiers compare by spelling (prefix and suffix included).
    #[test]
    fn literals_and_identifiers_by_spelling() {
        assert!(!eq("\"a\"", "\"b\""));
        assert!(!eq("u8\"a\"", "\"a\""));
        assert!(!eq("1.0", "1.00"));
        assert!(!eq("\\u00e9", "\u{e9}"), "identifiers compare by spelling, not by symbol");
        assert!(eq("R\"x(a\n  b)x\"", "R\"x(a\n  b)x\""));
        assert!(!eq("R\"x(a\n  b)x\"", "R\"x(a\nb)x\""), "raw string bodies are verbatim");
        assert!(!eq("x", "\"x\""));
    }

    #[test]
    fn report_shows_hunk_with_locations() {
        let d = diff(
            "int a = 1 ;\nint b = 2;\n",
            "int a = 1;\nint b = 3;\n",
            Std::Cxx26,
            false,
            ("expected", "actual"),
        )
        .unwrap();
        assert!(
            d.starts_with(
                "token streams differ at token 9 (expected: 10 tokens, actual: 10 tokens)"
            ),
            "{d}"
        );
        assert!(d.contains("-2:9  2  (pp-number)\n+2:9  3  (pp-number)\n"), "{d}");
        assert!(d.contains(" 2:5  b  (identifier)\n"), "{d}");
        assert!(d.contains("expected line 2: int b = 2;\nactual line 2: int b = 3;\n"), "{d}");
        // An insertion resyncs.
        let d = diff(
            "a b c d e f g h i j k l",
            "a b X c d e f g h i j k l",
            Std::Cxx26,
            false,
            ("oracle", "actual"),
        )
        .unwrap();
        assert!(d.contains("+1:5  X  (identifier)\n 1:5  c  (identifier)\n"), "{d}");
        assert!(!d.contains("-1:"), "{d}");
        // A truncated output.
        let d = diff("a b c", "a b", Std::Cxx26, false, ("expected", "actual")).unwrap();
        assert!(d.contains("-1:5  c  (identifier)\n+<end of actual>\n"), "{d}");
    }

    #[test]
    fn markers_give_presumed_locations() {
        let clang = "# 1 \"t.cpp\"\n# 1 \"<built-in>\" 1\n# 1 \"<built-in>\" 3\n# 465 \"<built-in>\" 3\n# 1 \"<command line>\" 1\n# 1 \"<built-in>\" 2\n# 1 \"t.cpp\" 2\nint a;\n\n\nint b;\n# 1 \"./h.h\" 1\nint c;\n# 15 \"t.cpp\" 2\nint d;\n";
        // The same tokens and locations, printed with fewer markers and
        // without the predefines pseudo-files.
        let other = "# 1 \"t.cpp\"\nint a;\n# 4 \"t.cpp\"\nint b;\n# 1 \"h.h\" 1\nint c;\n# 15 \"t.cpp\" 2\nint d;\n";
        assert!(eq_markers(clang, other), "{:?}", diff(clang, other, Std::Cxx26, true, ("a", "b")));
        // `int b` one line off.
        let off = other.replace("# 4 \"t.cpp\"", "# 5 \"t.cpp\"");
        let d = diff(clang, &off, Std::Cxx26, true, ("oracle", "actual")).unwrap();
        assert!(d.contains("-11:1 [t.cpp:4]  int  (identifier)"), "{d}");
        assert!(d.contains("+4:1 [t.cpp:5]  int  (identifier)"), "{d}");
        // A missing include enter marker.
        let no_enter = other.replace("# 1 \"h.h\" 1\n", "# 1 \"h.h\"\n");
        let d = diff(clang, &no_enter, Std::Cxx26, true, ("oracle", "actual")).unwrap();
        assert!(d.contains("-12:1  # 1 h.h 1  (line marker)"), "{d}");
        // Without marker mode the markers are just tokens and differ.
        assert!(!eq(clang, other));
        // System-header flags are part of a token's location.
        let sys = "# 1 \"s.h\" 3\nint x;\n";
        assert!(!eq_markers(sys, "# 1 \"s.h\"\nint x;\n"));
        assert!(eq_markers(
            "# 7 \"/a/b/../c/./d.h\" 3 4\nint x;\n",
            "# 7 \"/a/c/d.h\" 3 4\nint x;\n"
        ));
        // `#line` form, and a marker-looking line that is not one.
        assert!(eq_markers("#line 7 \"f\"\nx\n", "# 7 \"f\"\nx\n"));
        assert!(!eq_markers("# pragma once\n", "\n"));
    }

    #[test]
    fn path_normalization() {
        assert_eq!(
            normalize_path("\"/usr/lib/llvm-19/bin/../include/c++/v1/vector\""),
            "/usr/lib/llvm-19/include/c++/v1/vector"
        );
        assert_eq!(normalize_path("\"./a//b/./c\""), "a/b/c");
        assert_eq!(normalize_path("\"../x/../../y\""), "../../y");
        assert_eq!(normalize_path("\"<built-in>\""), "<built-in>");
    }

    #[test]
    fn std_mapping() {
        assert_eq!(lex_std("c++17"), Std::Cxx17);
        assert_eq!(lex_std("c++26"), Std::Cxx26);
        assert_eq!(lex_std("c17"), Std::Cxx98);
        // C++20 `<=>` is one token, before it is two.
        assert!(diff("a<=>b", "a<= >b", Std::Cxx20, false, ("e", "a")).is_some());
        assert!(diff("a<=>b", "a<= >b", Std::Cxx17, false, ("e", "a")).is_none());
    }
}
