//! Message templates (`spec/diag.md` section "Templates").
//!
//! - `{N}` is replaced by argument `N` (decimal, 0-based).
//! - `{N:alt0|alt1|...}` selects an alternative by argument `N`, read as
//!   a decimal index; a non-numeric or out-of-range value selects the last
//!   alternative. Alternatives may contain `{M}` placeholders but not
//!   nested selects.
//! - `{{` and `}}` are literal braces (outside alternatives).
//!
//! [`check`] is a `const fn` so that the id table can assert at compile
//! time that each template uses exactly the placeholders `0..nargs`.

/// True if `tmpl` is well-formed and uses every placeholder index in
/// `0..nargs` and no other.
///
/// The id table asserts this in a `const` item for every id, so a
/// mismatch is a build error:
///
/// ```compile_fail
/// const _: () = assert!(kiln_diag::template::check("'{0}' and '{1}'", 1));
/// ```
///
/// ```
/// const _: () = assert!(kiln_diag::template::check("'{0}' and '{1}'", 2));
/// ```
pub const fn check(tmpl: &str, nargs: usize) -> bool {
    if nargs > 64 {
        return false;
    }
    let b = tmpl.as_bytes();
    let mut used: u64 = 0;
    let mut in_select = false;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'{' => {
                if !in_select && i + 1 < b.len() && b[i + 1] == b'{' {
                    i += 2;
                    continue;
                }
                i += 1;
                let start = i;
                let mut n: usize = 0;
                while i < b.len() && b[i].is_ascii_digit() {
                    n = n * 10 + (b[i] - b'0') as usize;
                    if n >= 64 {
                        return false;
                    }
                    i += 1;
                }
                if i == start || n >= nargs || i >= b.len() {
                    return false;
                }
                used |= 1 << n;
                if b[i] == b'}' {
                    i += 1;
                } else if b[i] == b':' && !in_select {
                    in_select = true;
                    i += 1;
                } else {
                    return false;
                }
            }
            b'}' => {
                if in_select {
                    in_select = false;
                    i += 1;
                } else if i + 1 < b.len() && b[i + 1] == b'}' {
                    i += 2;
                } else {
                    return false;
                }
            }
            _ => i += 1,
        }
    }
    let all = if nargs == 64 { u64::MAX } else { (1u64 << nargs) - 1 };
    !in_select && used == all
}

/// Renders `tmpl` with `args`. Missing arguments render as nothing; a
/// malformed template renders its remaining text verbatim.
pub fn render(tmpl: &str, args: &[String]) -> String {
    let mut out = String::with_capacity(tmpl.len() + 16);
    let mut p = Parser { s: tmpl, i: 0 };
    p.seq(&mut out, args, false);
    // A stray `|` or `}` at top level cannot stop `seq`; nothing remains.
    out.push_str(&tmpl[p.i..]);
    out
}

struct Parser<'a> {
    s: &'a str,
    i: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.as_bytes().get(self.i).copied()
    }

    /// Renders until end of input or, inside an alternative, an unconsumed
    /// `|` or `}`.
    fn seq(&mut self, out: &mut String, args: &[String], in_alt: bool) {
        while let Some(c) = self.peek() {
            match c {
                b'|' | b'}' if in_alt => return,
                b'{' if !in_alt && self.s.as_bytes().get(self.i + 1) == Some(&b'{') => {
                    out.push('{');
                    self.i += 2;
                }
                b'}' if self.s.as_bytes().get(self.i + 1) == Some(&b'}') => {
                    out.push('}');
                    self.i += 2;
                }
                b'{' => {
                    if !self.placeholder(out, args, in_alt) {
                        out.push('{');
                        self.i += 1;
                    }
                }
                _ => {
                    // Copy a run of ordinary text (delimiters are ASCII, so
                    // the slice boundaries are char boundaries).
                    let start = self.i;
                    while let Some(c) = self.peek() {
                        if matches!(c, b'{' | b'}') || (in_alt && c == b'|') {
                            break;
                        }
                        self.i += 1;
                    }
                    if self.i == start {
                        // A lone `}` at top level: copy it.
                        self.i += 1;
                    }
                    out.push_str(&self.s[start..self.i]);
                }
            }
        }
    }

    /// Parses `{N}` or `{N:...}` at `self.i`; false (and no progress) if
    /// malformed.
    fn placeholder(&mut self, out: &mut String, args: &[String], in_alt: bool) -> bool {
        let b = self.s.as_bytes();
        let mut j = self.i + 1;
        let start = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        let Ok(n) = self.s[start..j].parse::<usize>() else {
            return false;
        };
        let arg = args.get(n).map_or("", String::as_str);
        match b.get(j) {
            Some(b'}') => {
                out.push_str(arg);
                self.i = j + 1;
                true
            }
            Some(b':') if !in_alt => {
                self.i = j + 1;
                let mut alts = Vec::new();
                loop {
                    let mut alt = String::new();
                    self.seq(&mut alt, args, true);
                    alts.push(alt);
                    match self.peek() {
                        Some(b'|') => self.i += 1,
                        Some(_) => {
                            self.i += 1;
                            break;
                        }
                        None => break,
                    }
                }
                let chosen = arg.parse::<usize>().ok().and_then(|k| alts.get(k));
                if let Some(s) = chosen.or(alts.last()) {
                    out.push_str(s);
                }
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_string()).collect()
    }

    #[test]
    fn check_counts() {
        assert!(check("plain", 0));
        assert!(check("{0} and {1}", 2));
        assert!(!check("{0}", 0));
        assert!(!check("{1}", 1));
        assert!(!check("{0}", 2));
        assert!(check("{0:a|b} {1}", 2));
        assert!(check("{0:a|{1}}", 2));
        assert!(!check("{0:a|{1:x|y}}", 2));
        assert!(check("{{literal}}", 0));
        assert!(!check("{0", 1));
        assert!(!check("}", 0));
        assert!(!check("{x}", 0));
    }

    #[test]
    fn render_basic() {
        assert_eq!(render("'{0}' macro redefined", &s(&["A"])), "'A' macro redefined");
        assert_eq!(render("{1}-{0}", &s(&["a", "b"])), "b-a");
        assert_eq!(render("{{x}}", &[]), "{x}");
        assert_eq!(render("too {0:few|many} args", &s(&["0"])), "too few args");
        assert_eq!(render("too {0:few|many} args", &s(&["1"])), "too many args");
        assert_eq!(render("too {0:few|many} args", &s(&["7"])), "too many args");
        assert_eq!(render("x{0:|; did you mean '{1}'?}", &s(&["0", "y"])), "x");
        assert_eq!(render("x{0:|; did you mean '{1}'?}", &s(&["1", "y"])), "x; did you mean 'y'?");
        assert_eq!(render("{0}", &[]), "");
        assert_eq!(render("é{0}ü", &s(&["ß"])), "éßü");
    }
}
