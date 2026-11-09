//! Sanitizer special case lists (`-fsanitize-ignorelist=` and the
//! resource directory's `share/*_ignorelist.txt`; WP-1032,
//! spec/driver.md "Sanitizers"): LLVM's `SpecialCaseList` format with
//! its glob patterns (`GlobPattern`) and clang's
//! `SanitizerSpecialCaseList` section masks and `NoSanitizeList` queries.
//!
//! A list is lines of `prefix:pattern[=category]` under optional
//! `[section]` headers; blank lines and lines starting with `#` are
//! skipped. A section's glob is matched against every sanitizer and
//! group name, and its mask is the union of the matching names' sets
//! (entries before any header are in section `*`, every sanitizer). A
//! query asks whether some section whose mask meets the asked sanitizers
//! has an entry of the prefix and category whose glob matches.
//!
//! The parse errors are LLVM's, word for word; the driver wraps them in
//! clang's `malformed sanitizer ignorelist: '...'`. A list starting with
//! `#!special-case-list-v1` uses POSIX regular expressions, which kiln
//! does not implement: [`IgnorelistError::Regex`].

use kiln_ast::sanitize::{GROUPS, SANITIZERS, SanitizerSet as San};

/// LLVM's limit on the subpatterns one glob's brace expansions make
/// (`SpecialCaseList::Matcher::insert`).
const MAX_SUB_PATTERNS: usize = 1024;

/// Why a list could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IgnorelistError {
    /// LLVM `SpecialCaseList::createInternal`'s message (`error parsing
    /// file '<path>': ...`).
    Malformed(String),
    /// A `#!special-case-list-v1` (regular expression) list: its path.
    Regex(String),
}

/// One `[section]` of one file.
#[derive(Clone, Debug)]
struct Section {
    /// The sanitizers its glob names.
    mask: San,
    /// The index of its file among the lists, in command-line order.
    file: u32,
    entries: Vec<Entry>,
}

/// One `prefix:pattern=category` line.
#[derive(Clone, Debug)]
struct Entry {
    prefix: Box<str>,
    category: Box<str>,
    glob: Glob,
    line: u32,
}

/// The parsed lists of a compilation (clang `NoSanitizeList`).
#[derive(Clone, Debug, Default)]
pub struct SanitizerIgnorelist {
    sections: Vec<Section>,
}

impl SanitizerIgnorelist {
    /// Parses the lists `files` (`(path, contents)`, in clang's order:
    /// the `-fsanitize-ignorelist=` files, then the default ones).
    ///
    /// # Errors
    /// The first malformed list, as LLVM words it.
    pub fn parse(files: &[(String, String)]) -> Result<SanitizerIgnorelist, IgnorelistError> {
        let mut l = SanitizerIgnorelist::default();
        for (i, (path, text)) in files.iter().enumerate() {
            if text.starts_with("#!special-case-list-v1\n") {
                return Err(IgnorelistError::Regex(path.clone()));
            }
            let file = u32::try_from(i).unwrap_or(u32::MAX);
            if let Err(e) = l.parse_one(text, file) {
                return Err(IgnorelistError::Malformed(format!(
                    "error parsing file '{path}': {e}"
                )));
            }
        }
        Ok(l)
    }

    /// LLVM `SpecialCaseList::parse` of one file.
    fn parse_one(&mut self, text: &str, file: u32) -> Result<(), String> {
        self.sections.push(Section { mask: San::ALL, file, entries: Vec::new() });
        for (n, raw) in text.split('\n').enumerate() {
            let line = u32::try_from(n + 1).unwrap_or(u32::MAX);
            // `line_iterator` skips lines whose first byte is the comment
            // marker; the rest is trimmed.
            if raw.starts_with('#') {
                continue;
            }
            let l = raw.trim_matches(|c: char| " \t\n\x0b\x0c\r".contains(c));
            if l.is_empty() {
                continue;
            }
            if let Some(rest) = l.strip_prefix('[') {
                let Some(name) = rest.strip_suffix(']') else {
                    return Err(format!("malformed section header on line {line}: {l}"));
                };
                let mask = section_mask(name).map_err(|e| {
                    format!("malformed section at line {line}: '{name}': {e}")
                })?;
                self.sections.push(Section { mask, file, entries: Vec::new() });
                continue;
            }
            let (prefix, postfix) = l.split_once(':').unwrap_or((l, ""));
            if postfix.is_empty() {
                return Err(format!("malformed line {line}: '{l}'"));
            }
            let (pattern, category) = postfix.split_once('=').unwrap_or((postfix, ""));
            let glob = if pattern.is_empty() {
                Err("Supplied glob was blank".to_owned())
            } else {
                Glob::new(pattern.as_bytes())
            }
            .map_err(|e| format!("malformed glob in line {line}: '{pattern}': {e}"))?;
            let s = self.sections.last_mut().expect("a section");
            s.entries.push(Entry { prefix: prefix.into(), category: category.into(), glob, line });
        }
        Ok(())
    }

    /// Whether there is no entry at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sections.iter().all(|s| s.entries.is_empty())
    }

    /// The last entry (`(file, line)`) of a section meeting `mask` with
    /// `prefix` and `category` whose glob matches `query` (LLVM 21+
    /// `inSectionBlame`).
    #[must_use]
    pub fn blame(&self, mask: San, prefix: &str, query: &str, category: &str) -> Option<(u32, u32)> {
        let mut best = None;
        for s in self.sections.iter().filter(|s| s.mask.has_any(mask)) {
            for e in &s.entries {
                if &*e.prefix == prefix
                    && &*e.category == category
                    && e.glob.matches(query.as_bytes())
                    && best.is_none_or(|b| (s.file, e.line) > b)
                {
                    best = Some((s.file, e.line));
                }
            }
        }
        best
    }

    /// clang `NoSanitizeList::containsPrefix`: whether `query` is listed
    /// under `prefix` (default category `category`) for a sanitizer of
    /// `mask`. With `sanitize_category` (clang 22) a later
    /// `prefix:...=sanitize` entry that also matches takes it back.
    #[must_use]
    pub fn contains(
        &self,
        mask: San,
        prefix: &str,
        query: &str,
        category: &str,
        sanitize_category: bool,
    ) -> bool {
        let Some(no) = self.blame(mask, prefix, query, category) else { return false };
        if !sanitize_category {
            return true;
        }
        self.blame(mask, prefix, query, "sanitize").is_none_or(|yes| no > yes)
    }
}

/// clang `SanitizerSpecialCaseList::createSanitizerSections`: the
/// sanitizers a section's glob names, or the glob's error.
fn section_mask(name: &str) -> Result<San, String> {
    if name.is_empty() {
        return Err("Supplied glob was blank".to_owned());
    }
    let g = Glob::new(name.as_bytes())?;
    Ok(SANITIZERS
        .iter()
        .chain(GROUPS)
        .filter(|(n, _)| g.matches(n.as_bytes()))
        .fold(San::NONE, |m, (_, s)| m.union(*s)))
}

/// LLVM `GlobPattern`: a literal prefix, then brace-expanded
/// subpatterns of `*`, `?`, `[...]` and `\` escapes.
#[derive(Clone, Debug)]
pub struct Glob {
    prefix: Vec<u8>,
    subs: Vec<SubGlob>,
}

/// One brace expansion of a glob's tail.
#[derive(Clone, Debug)]
struct SubGlob {
    pat: Vec<u8>,
    /// Each `[...]`: the offset after its `]` and its 256 byte bits.
    brackets: Vec<(usize, [u64; 4])>,
}

impl Glob {
    /// LLVM `GlobPattern::create(S, 1024)`.
    ///
    /// # Errors
    /// LLVM's message for a malformed pattern.
    pub fn new(s: &[u8]) -> Result<Glob, String> {
        let Some(p) = s.iter().position(|c| b"?*[{\\".contains(c)) else {
            return Ok(Glob { prefix: s.to_vec(), subs: Vec::new() });
        };
        let (prefix, rest) = s.split_at(p);
        let subs = brace_expansions(rest)?
            .into_iter()
            .map(|sp| SubGlob::new(&sp))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Glob { prefix: prefix.to_vec(), subs })
    }

    /// LLVM `GlobPattern::match`.
    #[must_use]
    pub fn matches(&self, s: &[u8]) -> bool {
        let Some(rest) = s.strip_prefix(self.prefix.as_slice()) else { return false };
        if self.subs.is_empty() && rest.is_empty() {
            return true;
        }
        self.subs.iter().any(|g| g.matches(rest))
    }
}

/// The index of the first `c` at or after `from` in `s`.
fn find(s: &[u8], c: u8, from: usize) -> Option<usize> {
    s.get(from..)?.iter().position(|&x| x == c).map(|i| i + from)
}

/// LLVM `parseBraceExpansions`.
fn brace_expansions(s: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    if !s.contains(&b'{') {
        return Ok(vec![s.to_vec()]);
    }
    // (start, length, terms)
    let mut bes: Vec<(usize, usize, Vec<&[u8]>)> = Vec::new();
    let mut open = false;
    let mut term_begin = 0;
    let mut i = 0;
    while i < s.len() {
        match s[i] {
            b'[' => {
                i = find(s, b']', i + 2)
                    .ok_or_else(|| "invalid glob pattern, unmatched '['".to_owned())?;
            }
            b'{' => {
                if open {
                    return Err("nested brace expansions are not supported".to_owned());
                }
                open = true;
                bes.push((i, 0, Vec::new()));
                term_begin = i + 1;
            }
            b',' if open => {
                let be = bes.last_mut().expect("an open expansion");
                be.2.push(&s[term_begin..i]);
                term_begin = i + 1;
            }
            b'}' if open => {
                let be = bes.last_mut().expect("an open expansion");
                if be.2.is_empty() {
                    return Err("empty or singleton brace expansions are not supported".to_owned());
                }
                be.2.push(&s[term_begin..i]);
                be.1 = i - be.0 + 1;
                open = false;
            }
            b'\\' => {
                i += 1;
                if i == s.len() {
                    return Err("invalid glob pattern, stray '\\'".to_owned());
                }
            }
            _ => {}
        }
        i += 1;
    }
    if open {
        return Err("incomplete brace expansion".to_owned());
    }
    let mut n: usize = 1;
    for be in &bes {
        n = n.saturating_mul(be.2.len());
    }
    if n > MAX_SUB_PATTERNS {
        return Err("too many brace expansions".to_owned());
    }
    let mut subs = vec![s.to_vec()];
    for (start, len, terms) in bes.iter().rev() {
        let orig = std::mem::take(&mut subs);
        for t in terms {
            for o in &orig {
                let mut v = o[..*start].to_vec();
                v.extend_from_slice(t);
                v.extend_from_slice(&o[start + len..]);
                subs.push(v);
            }
        }
    }
    Ok(subs)
}

/// LLVM's `expand` of a bracket's characters (`X-Y` ranges); `orig` is
/// the subpattern its error names.
fn expand(mut s: &[u8], orig: &[u8]) -> Result<[u64; 4], String> {
    let mut bv = [0u64; 4];
    let mut set = |c: u8| bv[usize::from(c >> 6)] |= 1 << (c & 63);
    while s.len() >= 3 {
        let (start, end) = (s[0], s[2]);
        if s[1] != b'-' {
            set(start);
            s = &s[1..];
            continue;
        }
        if start > end {
            return Err(format!("invalid glob pattern: {}", String::from_utf8_lossy(orig)));
        }
        for c in start..=end {
            set(c);
        }
        s = &s[3..];
    }
    for &c in s {
        set(c);
    }
    Ok(bv)
}

impl SubGlob {
    /// LLVM `SubGlobPattern::create`.
    fn new(s: &[u8]) -> Result<SubGlob, String> {
        let mut brackets = Vec::new();
        let mut i = 0;
        while i < s.len() {
            match s[i] {
                b'[' => {
                    // `]` may be the first character of a class.
                    i += 1;
                    let j = find(s, b']', i + 1)
                        .ok_or_else(|| "invalid glob pattern, unmatched '['".to_owned())?;
                    let chars = &s[i..j];
                    let invert = matches!(s[i], b'^' | b'!');
                    let mut bv = if invert { expand(&chars[1..], s)? } else { expand(chars, s)? };
                    if invert {
                        for w in &mut bv {
                            *w = !*w;
                        }
                    }
                    brackets.push((j + 1, bv));
                    i = j;
                }
                b'\\' => {
                    i += 1;
                    if i == s.len() {
                        return Err("invalid glob pattern, stray '\\'".to_owned());
                    }
                }
                _ => {}
            }
            i += 1;
        }
        Ok(SubGlob { pat: s.to_vec(), brackets })
    }

    /// LLVM `SubGlobPattern::match`: segments split by `*`, each matched
    /// at the first place past the previous one, backtracking on a
    /// mismatch.
    fn matches(&self, s: &[u8]) -> bool {
        let pat = &self.pat;
        let (mut p, mut si) = (0usize, 0usize);
        let (mut seg, mut saved_s, mut b, mut saved_b) = (None::<usize>, 0usize, 0usize, 0usize);
        while si < s.len() {
            if p < pat.len() {
                match pat[p] {
                    b'*' => {
                        p += 1;
                        seg = Some(p);
                        saved_s = si;
                        saved_b = b;
                        continue;
                    }
                    b'[' => {
                        let (next, bits) = self.brackets[b];
                        let c = s[si];
                        if bits[usize::from(c >> 6)] >> (c & 63) & 1 == 1 {
                            p = next;
                            b += 1;
                            si += 1;
                            continue;
                        }
                    }
                    b'\\' => {
                        p += 1;
                        if pat[p] == s[si] {
                            p += 1;
                            si += 1;
                            continue;
                        }
                    }
                    c if c == s[si] || c == b'?' => {
                        p += 1;
                        si += 1;
                        continue;
                    }
                    _ => {}
                }
            }
            let Some(sb) = seg else { return false };
            p = sb;
            saved_s += 1;
            si = saved_s;
            b = saved_b;
        }
        pat[p.min(pat.len())..].iter().all(|&c| c == b'*')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(p: &str) -> Glob {
        Glob::new(p.as_bytes()).unwrap()
    }

    fn err(p: &str) -> String {
        Glob::new(p.as_bytes()).unwrap_err()
    }

    #[test]
    fn globs_match_as_llvm() {
        assert!(g("abc").matches(b"abc"));
        assert!(!g("abc").matches(b"abcd"));
        assert!(g("*").matches(b""));
        assert!(g("*").matches(b"a/b/c"));
        assert!(g("_Z*add*").matches(b"_Z3addii"));
        assert!(!g("_Z*add*").matches(b"_Z3subii"));
        assert!(g("a?c").matches(b"abc"));
        assert!(g("*h.h").matches(b"./inc/h.h"));
        assert!(g("[a-c]x").matches(b"bx"));
        assert!(!g("[!a-c]x").matches(b"bx"));
        assert!(g("[^a-c]x").matches(b"dx"));
        assert!(g("[]]").matches(b"]"));
        assert!(g("\\*").matches(b"*"));
        assert!(!g("\\*").matches(b"a"));
        assert!(g("f{oo,ar}").matches(b"far"));
        assert!(!g("f{oo,ar}").matches(b"fxx"));
        assert!(g("*a*b").matches(b"xaxxab"));
        assert!(g("a**").matches(b"a"));
    }

    #[test]
    fn glob_errors_word_for_word() {
        assert_eq!(err("a[b"), "invalid glob pattern, unmatched '['");
        assert_eq!(err("a\\"), "invalid glob pattern, stray '\\'");
        assert_eq!(err("{a,{b,c}}"), "nested brace expansions are not supported");
        assert_eq!(err("{a}"), "empty or singleton brace expansions are not supported");
        assert_eq!(err("{a,b"), "incomplete brace expansion");
        assert_eq!(err("x[z-a]"), "invalid glob pattern: [z-a]");
        let many = "{a,b,c,d}".repeat(6);
        assert_eq!(err(&many), "too many brace expansions");
    }

    fn list(text: &str) -> Result<SanitizerIgnorelist, IgnorelistError> {
        SanitizerIgnorelist::parse(&[("l.txt".to_owned(), text.to_owned())])
    }

    fn malformed(text: &str) -> String {
        match list(text).unwrap_err() {
            IgnorelistError::Malformed(m) => m,
            IgnorelistError::Regex(_) => panic!("regex"),
        }
    }

    #[test]
    fn list_errors_word_for_word() {
        assert_eq!(malformed("# c\nbadline\n"), "error parsing file 'l.txt': malformed line 2: 'badline'");
        assert_eq!(
            malformed("[address\n"),
            "error parsing file 'l.txt': malformed section header on line 1: [address"
        );
        assert_eq!(
            malformed("\n[]\n"),
            "error parsing file 'l.txt': malformed section at line 2: '': Supplied glob was blank"
        );
        assert_eq!(
            malformed("fun:\n"),
            "error parsing file 'l.txt': malformed line 1: 'fun:'"
        );
        assert_eq!(
            malformed("fun:=init\n"),
            "error parsing file 'l.txt': malformed glob in line 1: '': Supplied glob was blank"
        );
        assert_eq!(
            malformed("src:a[\n"),
            "error parsing file 'l.txt': malformed glob in line 1: 'a[': invalid glob pattern, unmatched '['"
        );
        assert_eq!(
            list("#!special-case-list-v1\nfun:.*\n").unwrap_err(),
            IgnorelistError::Regex("l.txt".to_owned())
        );
    }

    #[test]
    fn sections_mask_their_sanitizers() {
        let l = list("fun:all\n[address]\nfun:asan\n[undefined]\nsrc:*h.h\n[{alignment,null}]\nfun:two=init\n").unwrap();
        assert!(l.contains(San::THREAD, "fun", "all", "", false));
        assert!(l.contains(San::ADDRESS, "fun", "asan", "", false));
        assert!(!l.contains(San::THREAD, "fun", "asan", "", false));
        assert!(l.contains(San::SIGNED_INTEGER_OVERFLOW, "src", "./inc/h.h", "", false));
        assert!(!l.contains(San::ADDRESS, "src", "./inc/h.h", "", false));
        assert!(l.contains(San::NULL, "fun", "two", "init", false));
        assert!(!l.contains(San::NULL, "fun", "two", "", false));
        assert!(!l.contains(San::SHIFT_BASE, "fun", "two", "init", false));
    }

    #[test]
    fn later_sanitize_entries_take_back() {
        let l = list("type:*\ntype:int=sanitize\n").unwrap();
        assert!(l.contains(San::SIGNED_INTEGER_OVERFLOW, "type", "long", "", true));
        assert!(!l.contains(San::SIGNED_INTEGER_OVERFLOW, "type", "int", "", true));
        assert!(l.contains(San::SIGNED_INTEGER_OVERFLOW, "type", "int", "", false));
        let l = list("type:int=sanitize\ntype:*\n").unwrap();
        assert!(l.contains(San::SIGNED_INTEGER_OVERFLOW, "type", "int", "", true));
    }
}
