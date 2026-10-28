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

/// `oven: steps<=N bytes<=M` — an evaluation budget (kiln-specific).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OvenBudget {
    pub steps: Option<u64>,
    pub bytes: Option<u64>,
}

/// `steps<=N bytes<=M` (either or both, any order).
pub fn parse_oven(value: &str) -> Result<OvenBudget, String> {
    let usage = "expected `oven: steps<=N bytes<=M` (at least one)";
    let mut b = OvenBudget::default();
    for item in value.split_whitespace() {
        let (key, n) =
            item.split_once("<=").ok_or_else(|| format!("bad budget `{item}`; {usage}"))?;
        let n: u64 = n.parse().map_err(|_| format!("bad number in `{item}`; {usage}"))?;
        let slot = match key {
            "steps" => &mut b.steps,
            "bytes" => &mut b.bytes,
            _ => return Err(format!("unknown budget `{key}`; {usage}")),
        };
        if slot.replace(n).is_some() {
            return Err(format!("duplicate budget `{key}`"));
        }
    }
    if b == OvenBudget::default() {
        return Err(usage.into());
    }
    Ok(b)
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
        assert_eq!(parse_oven("bytes<=7").unwrap(), OvenBudget { steps: None, bytes: Some(7) });
        assert!(parse_oven("").is_err());
        assert!(parse_oven("steps<=1 steps<=2").is_err());
        assert!(parse_oven("time<=3").is_err());
    }
}
