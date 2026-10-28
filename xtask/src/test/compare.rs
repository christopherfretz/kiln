//! Output comparison: preprocessed-output normalization and a line diff for
//! failure reports.

use std::fmt::Write as _;

use super::directives::PpCompare;
use super::pp_tokens;

/// Whitespace normalization for `-E -P` output (`spec/harness.md`): collapse
/// runs of spaces/tabs to one space, trim both ends of every line, drop blank
/// lines. Each kept line is `\n`-terminated.
pub fn normalize_pp(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for line in s.lines() {
        let mut first = true;
        for word in line.split([' ', '\t', '\r', '\x0b', '\x0c']).filter(|w| !w.is_empty()) {
            if !first {
                out.push(' ');
            }
            out.push_str(word);
            first = false;
        }
        if !first {
            out.push('\n');
        }
    }
    out
}

/// Compares two preprocessed outputs the way `how` says (`spec/harness.md`
/// "pp comparison"). `None` when they are equivalent, else a failure report
/// that starts with a one-line summary. `std` is the test's `-std=` value
/// (it selects the re-lexing rules); `markers` is set when the output was
/// produced without `-P`. `labels` name the two sides.
pub fn pp_diff(
    how: PpCompare,
    std: &str,
    markers: bool,
    expected: &str,
    actual: &str,
    labels: (&str, &str),
) -> Option<String> {
    match how {
        PpCompare::Tokens => {
            pp_tokens::diff(expected, actual, pp_tokens::lex_std(std), markers, labels)
        }
        PpCompare::Text => {
            let (a, b) = (normalize_pp(expected), normalize_pp(actual));
            (a != b).then(|| {
                format!(
                    "outputs differ after whitespace normalization (-{} +{}):\n{}",
                    labels.0,
                    labels.1,
                    line_diff(&a, &b)
                )
            })
        }
    }
}

/// A line diff of `expected` vs `actual`: `-` lines are missing from actual,
/// `+` lines are extra. Long unchanged runs are elided.
pub fn line_diff(expected: &str, actual: &str) -> String {
    const CTX: usize = 2;
    let a: Vec<&str> = expected.lines().collect();
    let b: Vec<&str> = actual.lines().collect();
    let mut out = String::new();
    if a.len().saturating_mul(b.len()) > 4_000_000 {
        // Too large for the quadratic LCS: report the first difference only.
        let i = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
        let _ = writeln!(
            out,
            "(output too large to diff: {} vs {} lines; first difference at line {})",
            a.len(),
            b.len(),
            i + 1
        );
        let _ = writeln!(out, "-{}", a.get(i).unwrap_or(&"<eof>"));
        let _ = writeln!(out, "+{}", b.get(i).unwrap_or(&"<eof>"));
        return out;
    }
    // lcs[i][j] = LCS length of a[i..] and b[j..].
    let mut lcs = vec![vec![0u32; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] =
                if a[i] == b[j] { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }
    let mut ops: Vec<(char, &str)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            ops.push((' ', a[i]));
            i += 1;
            j += 1;
        } else if i < a.len() && (j == b.len() || lcs[i + 1][j] >= lcs[i][j + 1]) {
            ops.push(('-', a[i]));
            i += 1;
        } else {
            ops.push(('+', b[j]));
            j += 1;
        }
    }
    // Keep CTX lines of context around changes.
    let changed: Vec<bool> = ops.iter().map(|(c, _)| *c != ' ').collect();
    let near = |k: usize| {
        let lo = k.saturating_sub(CTX);
        let hi = (k + CTX + 1).min(ops.len());
        changed[lo..hi].iter().any(|&c| c)
    };
    let mut elided = 0usize;
    for (k, (c, line)) in ops.iter().enumerate() {
        if near(k) {
            if elided > 0 {
                let _ = writeln!(out, " ... ({elided} unchanged lines)");
                elided = 0;
            }
            let _ = writeln!(out, "{c}{line}");
        } else {
            elided += 1;
        }
    }
    if elided > 0 {
        let _ = writeln!(out, " ... ({elided} unchanged lines)");
    }
    if expected.ends_with('\n') != actual.ends_with('\n')
        && !expected.is_empty()
        && !actual.is_empty()
    {
        out.push_str("(trailing newline differs)\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization() {
        assert_eq!(normalize_pp("  a \t+   b  \n\n\t\n c\r\n"), "a + b\nc\n");
        assert_eq!(normalize_pp(""), "");
        let text = |a: &str, b: &str| pp_diff(PpCompare::Text, "c++26", false, a, b, ("e", "a"));
        assert_eq!(text("x  y\n\n", "\n   x y"), None);
        assert!(
            text("x y", "xy")
                .unwrap()
                .starts_with("outputs differ after whitespace normalization (-e +a):\n")
        );
        // The text rule sees spacing the token rule ignores.
        assert!(text("a+b", "a + b").is_some());
        assert_eq!(pp_diff(PpCompare::Tokens, "c++26", false, "a+b", "a + b", ("e", "a")), None);
    }

    #[test]
    fn diff_shows_changes_with_context() {
        let e = "1\n2\n3\n4\n5\n6\n7\n8\n9\n";
        let a = "1\n2\n3\n4\nFIVE\n6\n7\n8\n9\n10\n";
        let d = line_diff(e, a);
        assert_eq!(d, " ... (2 unchanged lines)\n 3\n 4\n-5\n+FIVE\n 6\n 7\n 8\n 9\n+10\n");
        let e: String = (0..20).map(|i| i.to_string() + "\n").collect();
        let a: String = (0..20)
            .map(|i| match i {
                3 => "x\n".to_owned(),
                15 => "y\n".to_owned(),
                _ => format!("{i}\n"),
            })
            .collect();
        assert_eq!(
            line_diff(&e, &a),
            " ... (1 unchanged lines)\n 1\n 2\n-3\n+x\n 4\n 5\n ... (7 unchanged lines)\n 13\n 14\n-15\n+y\n 16\n 17\n ... (2 unchanged lines)\n"
        );
    }

    #[test]
    fn diff_edges() {
        assert_eq!(line_diff("", "x\n"), "+x\n");
        assert_eq!(line_diff("x\n", ""), "-x\n");
        assert_eq!(line_diff("x\n", "x"), " ... (1 unchanged lines)\n(trailing newline differs)\n");
    }
}
