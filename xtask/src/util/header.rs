//! The `//@` directive header shared by `cargo xtask test` (`spec/harness.md`
//! "Directives") and `cargo xtask bench` (`spec/bench.md`).
//!
//! The header is the leading run of lines that are blank or `//` comments.
//! Every line in it whose trimmed text starts with `//@` is a directive of the
//! form `key: value` or (for flags) a bare `key`. A `//@` line after the header
//! is an error, so a misplaced directive can never be silently ignored. What
//! the keys mean is up to the caller; this module only finds them and provides
//! the value parsers both runners share.

use std::fmt;

/// One `//@` line of the header, split at the first `:`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawDirective<'s> {
    /// 1-based source line.
    pub line: usize,
    /// The trimmed text before the first `:` (or the whole body for a flag).
    pub key: &'s str,
    /// The trimmed text after the first `:`; `None` for a bare flag.
    pub value: Option<&'s str>,
}

/// A malformed directive header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectiveError {
    /// 1-based source line.
    pub line: usize,
    pub message: String,
}

impl fmt::Display for DirectiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

/// The `std:` values accepted for C++ sources.
pub const CXX_STDS: &[&str] =
    &["c++98", "c++03", "c++11", "c++14", "c++17", "c++20", "c++23", "c++26"];
/// The `std:` values accepted for C sources.
pub const C_STDS: &[&str] = &["c89", "c99", "c11", "c17", "c23"];

/// Finds the directives of `source`'s header. A `//@` line after the header
/// is reported as an error (and not returned as a directive).
pub fn scan(source: &str) -> (Vec<RawDirective<'_>>, Vec<DirectiveError>) {
    let mut directives = Vec::new();
    let mut errors = Vec::new();
    let mut in_header = true;
    for (idx, raw) in source.lines().enumerate() {
        let line = idx + 1;
        let text = raw.trim();
        if in_header && !(text.is_empty() || text.starts_with("//")) {
            in_header = false;
        }
        let Some(body) = text.strip_prefix("//@") else {
            continue;
        };
        if in_header {
            let (key, value) = match body.split_once(':') {
                Some((k, v)) => (k.trim(), Some(v.trim())),
                None => (body.trim(), None),
            };
            directives.push(RawDirective { line, key, value });
        } else {
            errors.push(DirectiveError {
                line,
                message: "`//@` directive after the header (directives must precede the first \
                          line of code)"
                    .into(),
            });
        }
    }
    (directives, errors)
}

/// A counter of the constexpr report (`spec/oven.md` "Reporting protocol")
/// that an `oven:` budget can bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Counter {
    Steps,
    Bytes,
    PeakBytes,
    Calls,
    Allocs,
    Evaluations,
}

impl Counter {
    pub const ALL: [Self; 6] =
        [Self::Steps, Self::Bytes, Self::PeakBytes, Self::Calls, Self::Allocs, Self::Evaluations];

    /// The report's field name (and the budget key).
    pub fn name(self) -> &'static str {
        match self {
            Self::Steps => "steps",
            Self::Bytes => "bytes",
            Self::PeakBytes => "peak_bytes",
            Self::Calls => "calls",
            Self::Allocs => "allocs",
            Self::Evaluations => "evaluations",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == s)
    }
}

/// Where a located `oven:` budget applies: evaluations reported at this
/// line (and column, if given) of the file the directive is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OvenLoc {
    pub line: u32,
    pub col: Option<u32>,
}

impl fmt::Display for OvenLoc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.col {
            Some(c) => write!(f, "{}:{c}", self.line),
            None => write!(f, "{}", self.line),
        }
    }
}

/// `oven: [<line>[:<col>]] <counter><=N...` — an evaluation budget
/// (kiln-specific). Without a location it bounds the report's total line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OvenBudget {
    pub at: Option<OvenLoc>,
    /// At least one bound, sorted by counter, each counter at most once.
    pub limits: Vec<(Counter, u64)>,
}

/// `[<line>[:<col>]] <counter><=N...` (any order; at least one bound).
pub fn parse_oven(value: &str) -> Result<OvenBudget, String> {
    let usage = "expected `oven: [<line>[:<col>]] <counter><=N...` with counters \
                 steps|bytes|peak_bytes|calls|allocs|evaluations";
    let mut items = value.split_whitespace().peekable();
    let at = match items.peek() {
        Some(first) if first.starts_with(|c: char| c.is_ascii_digit()) => {
            let first = items.next().unwrap_or_default();
            Some(parse_loc(first).ok_or_else(|| format!("bad location `{first}`; {usage}"))?)
        }
        _ => None,
    };
    let mut limits: Vec<(Counter, u64)> = Vec::new();
    for item in items {
        let (key, n) =
            item.split_once("<=").ok_or_else(|| format!("bad budget `{item}`; {usage}"))?;
        let n: u64 = n.parse().map_err(|_| format!("bad number in `{item}`; {usage}"))?;
        let c = Counter::parse(key).ok_or_else(|| format!("unknown budget `{key}`; {usage}"))?;
        if limits.iter().any(|(k, _)| *k == c) {
            return Err(format!("duplicate budget `{key}`"));
        }
        limits.push((c, n));
    }
    if limits.is_empty() {
        return Err(usage.into());
    }
    limits.sort();
    Ok(OvenBudget { at, limits })
}

/// `<line>[:<col>]`, both 1-based.
fn parse_loc(s: &str) -> Option<OvenLoc> {
    let num = |t: &str| t.parse::<u32>().ok().filter(|&n| n > 0);
    match s.split_once(':') {
        Some((l, c)) => Some(OvenLoc { line: num(l)?, col: Some(num(c)?) }),
        None => Some(OvenLoc { line: num(s)?, col: None }),
    }
}

/// Splits `args:` like a POSIX shell would, minus expansion: whitespace
/// separates words, `'...'` is literal, `"..."` allows `\"` and `\\`, and an
/// unquoted `\` escapes the next character.
pub fn split_args(s: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                if in_word {
                    out.push(std::mem::take(&mut cur));
                    in_word = false;
                }
            }
            '\'' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(c) => cur.push(c),
                        None => return Err("unterminated `'` in args".into()),
                    }
                }
            }
            '"' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(c @ ('"' | '\\')) => cur.push(c),
                            Some(c) => {
                                cur.push('\\');
                                cur.push(c);
                            }
                            None => return Err("unterminated `\"` in args".into()),
                        },
                        Some(c) => cur.push(c),
                        None => return Err("unterminated `\"` in args".into()),
                    }
                }
            }
            '\\' => {
                in_word = true;
                match chars.next() {
                    Some(c) => cur.push(c),
                    None => return Err("trailing `\\` in args".into()),
                }
            }
            c => {
                in_word = true;
                cur.push(c);
            }
        }
    }
    if in_word {
        out.push(cur);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_splits_keys_and_flags() {
        let (d, e) =
            scan("// comment\n//@ std: c++20\n\n  //@ strict-diags\n//@ args: a:b\nint x;\n");
        assert_eq!(e, []);
        assert_eq!(
            d,
            [
                RawDirective { line: 2, key: "std", value: Some("c++20") },
                RawDirective { line: 4, key: "strict-diags", value: None },
                RawDirective { line: 5, key: "args", value: Some("a:b") },
            ]
        );
    }

    #[test]
    fn scan_rejects_late_directives() {
        let (d, e) = scan("//@ std: c++20\nint x;\n//@ args: -O2\n");
        assert_eq!(d.len(), 1);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].line, 3);
        assert!(e[0].message.contains("after the header"));
    }

    #[test]
    fn oven_budgets() {
        assert_eq!(
            parse_oven("bytes<=7").unwrap(),
            OvenBudget { at: None, limits: vec![(Counter::Bytes, 7)] }
        );
        let b = parse_oven("evaluations<=2 steps<=10 peak_bytes<=3 calls<=4 allocs<=5").unwrap();
        assert_eq!(b.at, None);
        assert_eq!(
            b.limits,
            [
                (Counter::Steps, 10),
                (Counter::PeakBytes, 3),
                (Counter::Calls, 4),
                (Counter::Allocs, 5),
                (Counter::Evaluations, 2)
            ]
        );
        let b = parse_oven("12:5 steps<=1").unwrap();
        assert_eq!(b.at, Some(OvenLoc { line: 12, col: Some(5) }));
        assert_eq!(b.at.unwrap().to_string(), "12:5");
        let b = parse_oven("7 bytes<=0").unwrap();
        assert_eq!(b.at, Some(OvenLoc { line: 7, col: None }));
        assert_eq!(b.limits, [(Counter::Bytes, 0)]);
        for bad in ["", "12:1", "steps<=1 steps<=2", "time<=3", "0:1 steps<=1", "3:x steps<=1"] {
            assert!(parse_oven(bad).is_err(), "{bad}");
        }
        assert!(parse_oven("steps<=1 12 bytes<=2").unwrap_err().contains("bad budget"));
        for c in Counter::ALL {
            assert_eq!(Counter::parse(c.name()), Some(c));
        }
    }
}
