//! lit's boolean expressions (`// REQUIRES:`, `// UNSUPPORTED:`, `// XFAIL:`,
//! `spec/libcxx.md` "Directives"), evaluated against a static feature set.
//!
//! Grammar (lit's `BooleanExpression`):
//!
//! ```text
//! expr  := or
//! or    := and ('||' and)*
//! and   := not ('&&' not)*
//! not   := '!' not | '(' or ')' | ident
//! ident := ( [-+=._a-zA-Z0-9]+ | '{{' regex '}}' )+
//! ```
//!
//! `true` is true. Any other identifier is true when it matches a whole
//! feature name, its literal parts exactly and its `{{regex}}` parts as a
//! regular expression in the subset of [`Regex`].

/// A parse error, with the expression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExprError(pub String);

#[derive(Clone, Debug, PartialEq, Eq)]
enum Tok {
    LParen,
    RParen,
    And,
    Or,
    Not,
    Ident(String),
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "-+=._".contains(c)
}

fn tokenize(s: &str) -> Result<Vec<Tok>, ExprError> {
    let err = |why: &str| ExprError(format!("{why} in `{s}`"));
    let mut out = Vec::new();
    let mut rest = s;
    loop {
        rest = rest.trim_start();
        let Some(c) = rest.chars().next() else { break };
        let (tok, len) = match c {
            '(' => (Tok::LParen, 1),
            ')' => (Tok::RParen, 1),
            '!' => (Tok::Not, 1),
            '&' if rest.starts_with("&&") => (Tok::And, 2),
            '|' if rest.starts_with("||") => (Tok::Or, 2),
            _ => {
                let mut len = 0;
                loop {
                    let r = &rest[len..];
                    if let Some(inner) = r.strip_prefix("{{") {
                        let Some(end) = inner.find("}}") else {
                            return Err(err("unterminated `{{`"));
                        };
                        len += end + 4;
                    } else if let Some(ch) = r.chars().next().filter(|&ch| is_ident_char(ch)) {
                        len += ch.len_utf8();
                    } else {
                        break;
                    }
                }
                if len == 0 {
                    return Err(err(&format!("unexpected `{c}`")));
                }
                (Tok::Ident(rest[..len].to_owned()), len)
            }
        };
        out.push(tok);
        rest = &rest[len..];
    }
    Ok(out)
}

/// A parsed expression.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    True,
    /// An identifier: literal pieces and `{{regex}}` pieces, matched against
    /// a whole feature name.
    Ident(Vec<Piece>),
    Not(Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Piece {
    Lit(String),
    Re(Regex),
}

impl Expr {
    /// Parses one boolean expression.
    pub fn parse(s: &str) -> Result<Self, ExprError> {
        let toks = tokenize(s)?;
        if toks.is_empty() {
            return Err(ExprError(format!("empty expression `{s}`")));
        }
        let mut p = Parser { toks: &toks, pos: 0, src: s };
        let e = p.or()?;
        if p.pos != toks.len() {
            return Err(ExprError(format!("unexpected token after expression in `{s}`")));
        }
        Ok(e)
    }

    /// Evaluates against `features`.
    pub fn eval(&self, features: &[&str]) -> bool {
        match self {
            Self::True => true,
            Self::Ident(pieces) => features.iter().any(|f| ident_matches(pieces, f)),
            Self::Not(e) => !e.eval(features),
            Self::And(a, b) => a.eval(features) && b.eval(features),
            Self::Or(a, b) => a.eval(features) || b.eval(features),
        }
    }
}

struct Parser<'a> {
    toks: &'a [Tok],
    pos: usize,
    src: &'a str,
}

impl Parser<'_> {
    fn err(&self, why: &str) -> ExprError {
        ExprError(format!("{why} in `{}`", self.src))
    }

    fn or(&mut self) -> Result<Expr, ExprError> {
        let mut e = self.and()?;
        while self.toks.get(self.pos) == Some(&Tok::Or) {
            self.pos += 1;
            e = Expr::Or(Box::new(e), Box::new(self.and()?));
        }
        Ok(e)
    }

    fn and(&mut self) -> Result<Expr, ExprError> {
        let mut e = self.not()?;
        while self.toks.get(self.pos) == Some(&Tok::And) {
            self.pos += 1;
            e = Expr::And(Box::new(e), Box::new(self.not()?));
        }
        Ok(e)
    }

    fn not(&mut self) -> Result<Expr, ExprError> {
        let tok = self.toks.get(self.pos).cloned().ok_or_else(|| self.err("missing operand"))?;
        self.pos += 1;
        match tok {
            Tok::Not => Ok(Expr::Not(Box::new(self.not()?))),
            Tok::LParen => {
                let e = self.or()?;
                if self.toks.get(self.pos) != Some(&Tok::RParen) {
                    return Err(self.err("missing `)`"));
                }
                self.pos += 1;
                Ok(e)
            }
            Tok::Ident(s) if s == "true" => Ok(Expr::True),
            Tok::Ident(s) => Ok(Expr::Ident(pieces(&s).map_err(|e| self.err(&e))?)),
            Tok::RParen | Tok::And | Tok::Or => Err(self.err("expected an operand")),
        }
    }
}

fn pieces(ident: &str) -> Result<Vec<Piece>, String> {
    let mut out = Vec::new();
    let mut rest = ident;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("{{") {
            let end = r.find("}}").ok_or("unterminated `{{`")?;
            out.push(Piece::Re(Regex::parse(&r[..end])?));
            rest = &r[end + 2..];
        } else {
            let end = rest.find("{{").unwrap_or(rest.len());
            out.push(Piece::Lit(rest[..end].to_owned()));
            rest = &rest[end..];
        }
    }
    Ok(out)
}

/// Does the whole of `s` match the identifier's pieces in sequence?
fn ident_matches(pieces: &[Piece], s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    let mut positions = vec![0usize];
    for p in pieces {
        let mut next = Vec::new();
        for &at in &positions {
            match p {
                Piece::Lit(l) => {
                    let l: Vec<char> = l.chars().collect();
                    if chars[at..].starts_with(&l) {
                        next.push(at + l.len());
                    }
                }
                Piece::Re(re) => next.extend(re.ends(&chars, at)),
            }
        }
        next.sort_unstable();
        next.dedup();
        positions = next;
        if positions.is_empty() {
            return false;
        }
    }
    positions.contains(&chars.len())
}

/// The regular-expression subset of `{{...}}` in lit identifiers: literal
/// characters, `\`-escapes, `.`, bracket classes (`[a-z]`, `[^0-9]`),
/// groups `( )` with alternation `|`, and the quantifiers `*`, `+`, `?`.
/// Anything else (`{n,m}`, anchors, backreferences) is a parse error, so an
/// unsupported pattern is reported, never silently misread.
#[derive(Clone, Debug, PartialEq)]
pub struct Regex(Alt);

#[derive(Clone, Debug, PartialEq)]
struct Alt(Vec<Vec<Node>>);

#[derive(Clone, Debug, PartialEq)]
enum Node {
    Char(char),
    Any,
    Class { negated: bool, ranges: Vec<(char, char)> },
    Group(Alt),
    Repeat { node: Box<Node>, min: usize, max_one: bool },
}

impl Regex {
    pub fn parse(s: &str) -> Result<Self, String> {
        let chars: Vec<char> = s.chars().collect();
        let mut pos = 0;
        let alt = parse_alt(&chars, &mut pos)?;
        if pos != chars.len() {
            return Err(format!("unbalanced `)` in regex `{s}`"));
        }
        Ok(Self(alt))
    }

    /// Does the regex match the whole of `s`?
    #[cfg(test)]
    pub fn full_match(&self, s: &str) -> bool {
        let chars: Vec<char> = s.chars().collect();
        self.ends(&chars, 0).contains(&chars.len())
    }

    /// Every position at which a match starting at `at` can end.
    fn ends(&self, s: &[char], at: usize) -> Vec<usize> {
        alt_ends(&self.0, s, at)
    }
}

fn parse_alt(c: &[char], pos: &mut usize) -> Result<Alt, String> {
    let mut alts = vec![Vec::new()];
    while let Some(&ch) = c.get(*pos) {
        match ch {
            ')' => break,
            '|' => {
                *pos += 1;
                alts.push(Vec::new());
                continue;
            }
            _ => {}
        }
        let atom = parse_atom(c, pos)?;
        let atom = match c.get(*pos) {
            Some('*') => Node::Repeat { node: Box::new(atom), min: 0, max_one: false },
            Some('+') => Node::Repeat { node: Box::new(atom), min: 1, max_one: false },
            Some('?') => Node::Repeat { node: Box::new(atom), min: 0, max_one: true },
            Some('{') => return Err("`{n,m}` quantifiers are not supported".into()),
            _ => {
                alts.last_mut().expect("non-empty").push(atom);
                continue;
            }
        };
        *pos += 1;
        alts.last_mut().expect("non-empty").push(atom);
    }
    Ok(Alt(alts))
}

fn parse_atom(c: &[char], pos: &mut usize) -> Result<Node, String> {
    let ch = c[*pos];
    *pos += 1;
    Ok(match ch {
        '.' => Node::Any,
        '\\' => {
            let e = *c.get(*pos).ok_or("trailing `\\` in regex")?;
            *pos += 1;
            if e.is_ascii_alphanumeric() {
                return Err(format!("escape `\\{e}` is not supported"));
            }
            Node::Char(e)
        }
        '(' => {
            if c.get(*pos) == Some(&'?') {
                return Err("`(?` groups are not supported".into());
            }
            let alt = parse_alt(c, pos)?;
            if c.get(*pos) != Some(&')') {
                return Err("missing `)` in regex".into());
            }
            *pos += 1;
            Node::Group(alt)
        }
        '[' => {
            let negated = c.get(*pos) == Some(&'^');
            if negated {
                *pos += 1;
            }
            let mut ranges = Vec::new();
            let mut first = true;
            loop {
                let lo = *c.get(*pos).ok_or("missing `]` in regex")?;
                *pos += 1;
                if lo == ']' && !first {
                    break;
                }
                first = false;
                let lo = if lo == '\\' {
                    let e = *c.get(*pos).ok_or("trailing `\\` in regex")?;
                    *pos += 1;
                    e
                } else {
                    lo
                };
                if c.get(*pos) == Some(&'-') && c.get(*pos + 1).is_some_and(|&h| h != ']') {
                    let hi = c[*pos + 1];
                    *pos += 2;
                    ranges.push((lo, hi));
                } else {
                    ranges.push((lo, lo));
                }
            }
            Node::Class { negated, ranges }
        }
        '^' | '$' => return Err(format!("anchor `{ch}` is not supported")),
        '*' | '+' | '?' => return Err(format!("quantifier `{ch}` without an operand")),
        _ => Node::Char(ch),
    })
}

fn alt_ends(alt: &Alt, s: &[char], at: usize) -> Vec<usize> {
    let mut out = Vec::new();
    for seq in &alt.0 {
        let mut positions = vec![at];
        for node in seq {
            let mut next = Vec::new();
            for &p in &positions {
                next.extend(node_ends(node, s, p));
            }
            next.sort_unstable();
            next.dedup();
            positions = next;
        }
        out.extend(positions);
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn node_ends(node: &Node, s: &[char], at: usize) -> Vec<usize> {
    match node {
        Node::Char(c) => (s.get(at) == Some(c)).then_some(at + 1).into_iter().collect(),
        Node::Any => (at < s.len()).then_some(at + 1).into_iter().collect(),
        Node::Class { negated, ranges } => s
            .get(at)
            .filter(|&&ch| ranges.iter().any(|&(lo, hi)| (lo..=hi).contains(&ch)) != *negated)
            .map(|_| at + 1)
            .into_iter()
            .collect(),
        Node::Group(alt) => alt_ends(alt, s, at),
        Node::Repeat { node, min, max_one } => {
            // Breadth-first closure over repetitions; positions only grow
            // (an empty iteration adds nothing new), so this terminates.
            let mut reached = vec![at];
            let mut frontier = vec![at];
            let mut out = if *min == 0 { vec![at] } else { Vec::new() };
            let mut count = 0;
            while !frontier.is_empty() {
                count += 1;
                let mut next = Vec::new();
                for &p in &frontier {
                    for e in node_ends(node, s, p) {
                        if count >= *min {
                            out.push(e);
                        }
                        if !reached.contains(&e) {
                            reached.push(e);
                            next.push(e);
                        }
                    }
                }
                if *max_one {
                    break;
                }
                frontier = next;
            }
            out.sort_unstable();
            out.dedup();
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEATURES: &[&str] = &[
        "c++26",
        "linux",
        "target=aarch64-unknown-linux-gnu",
        "libcpp-abi-version=1",
        "stdlib=libc++",
        "kiln",
    ];

    #[test]
    fn expressions() {
        let cases: &[(&str, bool)] = &[
            ("true", true),
            ("c++26", true),
            ("c++03", false),
            ("!c++03", true),
            ("c++03 || c++26", true),
            ("c++03 && c++26", false),
            ("!(c++03 || c++11) && linux", true),
            ("clang", false),
            ("gcc-13 || clang-17", false),
            ("kiln", true),
            ("!kiln", false),
            ("target={{.+}}-linux-gnu", true),
            ("target={{.+}}-apple-{{.+}}", false),
            ("target=aarch64{{.*}}", true),
            ("target={{x86_64-.*}}", false),
            ("target={{aarch64|armv7}}-unknown-linux-gnu", true),
            ("libcpp-abi-version={{1|2}}", true),
            ("libcpp-abi-version={{[2-9]}}", false),
            ("c++{{2[0-6]}}", true),
            ("c++{{(0|1)[0-9]}}", false),
            ("stdlib=libc++ && !stdlib=apple-libc++", true),
            // Precedence: && binds tighter than ||.
            ("linux || c++03 && c++11", true),
            ("c++03 && c++11 || kiln", true),
            ("!linux || !kiln", false),
            // Whole-name matching: a prefix is not a match.
            ("c++2", false),
            ("linu", false),
        ];
        for &(src, want) in cases {
            let e = Expr::parse(src).unwrap_or_else(|e| panic!("{src}: {e:?}"));
            assert_eq!(e.eval(FEATURES), want, "{src}");
        }
    }

    #[test]
    fn parse_errors() {
        for src in ["", "&&", "a &&", "(a", "a)", "a b", "{{x", "a{{x{2}}}", "a || || b", "a & b"] {
            assert!(Expr::parse(src).is_err(), "`{src}` should not parse");
        }
    }

    #[test]
    fn regex_subset() {
        let cases: &[(&str, &str, bool)] = &[
            (".+", "x", true),
            (".+", "", false),
            (".*", "", true),
            ("(.+)?", "", true),
            ("(eabi)?(21|22)", "eabi21", true),
            ("(eabi)?(21|22)", "22", true),
            ("(eabi)?(21|22)", "eabi", false),
            ("(13|14)(.0)?", "14.0", true),
            ("(13|14)(.0)?", "14x0", true),
            ("(13|14)(.0)?", "15", false),
            ("arm64|arm64e|armv(7|8)(l|m)?", "armv7l", true),
            ("arm64|arm64e|armv(7|8)(l|m)?", "armv9", false),
            ("7.2.*|7.3.0.*", "7.3.0.1", true),
            ("[^0-9]+", "abc", true),
            ("[^0-9]+", "a1", false),
            ("a\\.b", "a.b", true),
            ("a\\.b", "axb", false),
            ("(a*)*b", "aaab", true),
        ];
        for &(re, s, want) in cases {
            let r = Regex::parse(re).unwrap_or_else(|e| panic!("{re}: {e}"));
            assert_eq!(r.full_match(s), want, "{re} on {s}");
        }
        for re in ["a{2}", "^a", "a$", "(?:a)", "\\d", "*a", "(a", "a)", "[ab"] {
            assert!(Regex::parse(re).is_err(), "`{re}` should be rejected");
        }
    }
}
