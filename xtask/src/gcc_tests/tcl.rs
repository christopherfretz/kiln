//! The slice of Tcl that GCC's testsuite metadata is written in: the word
//! and list syntax of `{ dg-... }` directives and of the `.x` files, and
//! Tcl's `string match` globs (spec/harness.md "GCC testsuites").
//!
//! Words are kept unsubstituted: a braced word is literal by Tcl's rules,
//! and the `.x` interpreter ([`super::xfile`]) substitutes `$var` and
//! `[cmd]` in quoted and bare words itself.

/// One word of a command or one element of a list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Word {
    /// `{...}`, the text between the outer braces.
    Braced(String),
    /// `"..."`, the text between the quotes (escapes not processed).
    Quoted(String),
    /// Anything else, up to white space outside brackets.
    Bare(String),
}

impl Word {
    /// The word's text without its delimiters.
    pub fn text(&self) -> &str {
        match self {
            Self::Braced(s) | Self::Quoted(s) | Self::Bare(s) => s,
        }
    }
}

/// The elements of a Tcl list (white space, newlines included, separates
/// elements; braces and quotes group).
pub fn list(s: &str) -> Vec<Word> {
    let mut sc = Scanner { c: s.chars().collect(), i: 0 };
    let mut out = Vec::new();
    loop {
        sc.skip(char::is_whitespace);
        if sc.at_end() {
            return out;
        }
        out.push(sc.word());
    }
}

/// [`list`] as plain strings.
pub fn list_strings(s: &str) -> Vec<String> {
    list(s).into_iter().map(|w| w.text().to_owned()).collect()
}

/// The commands of a Tcl script, each as its words: a newline or `;`
/// outside braces, quotes and brackets ends a command, `#` where a command
/// starts begins a comment to the end of the line, and a backslash-newline
/// is white space.
pub fn commands(script: &str) -> Vec<Vec<Word>> {
    let mut sc = Scanner { c: script.chars().collect(), i: 0 };
    let mut out = Vec::new();
    let mut cur: Vec<Word> = Vec::new();
    loop {
        sc.skip(|c| c == ' ' || c == '\t' || c == '\r');
        if sc.eat_str("\\\n") {
            continue;
        }
        match sc.peek() {
            None => break,
            Some('\n' | ';') => {
                sc.i += 1;
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            Some('#') if cur.is_empty() => sc.skip(|c| c != '\n'),
            Some(_) => cur.push(sc.word()),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

struct Scanner {
    c: Vec<char>,
    i: usize,
}

impl Scanner {
    fn at_end(&self) -> bool {
        self.i >= self.c.len()
    }

    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }

    fn skip(&mut self, f: impl Fn(char) -> bool) {
        while self.peek().is_some_and(&f) {
            self.i += 1;
        }
    }

    fn eat_str(&mut self, s: &str) -> bool {
        let n = s.chars().count();
        if self.c.len() >= self.i + n && self.c[self.i..self.i + n].iter().copied().eq(s.chars()) {
            self.i += n;
            true
        } else {
            false
        }
    }

    /// One word starting at the current (non-blank) character.
    fn word(&mut self) -> Word {
        match self.peek() {
            Some('{') => {
                self.i += 1;
                let start = self.i;
                let mut depth = 1;
                while let Some(c) = self.peek() {
                    self.i += 1;
                    match c {
                        '\\' => self.i += 1,
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                let end = self.i - 1;
                                return Word::Braced(self.c[start..end].iter().collect());
                            }
                        }
                        _ => {}
                    }
                }
                Word::Braced(self.c[start..].iter().collect())
            }
            Some('"') => {
                self.i += 1;
                let start = self.i;
                let mut brackets = 0usize;
                while let Some(c) = self.peek() {
                    match c {
                        '\\' => self.i += 1,
                        '[' => brackets += 1,
                        ']' => brackets = brackets.saturating_sub(1),
                        '"' if brackets == 0 => {
                            let w = Word::Quoted(self.c[start..self.i].iter().collect());
                            self.i += 1;
                            return w;
                        }
                        _ => {}
                    }
                    self.i += 1;
                }
                Word::Quoted(self.c[start.min(self.c.len())..].iter().collect())
            }
            _ => {
                let start = self.i;
                let mut brackets = 0usize;
                let mut quoted = false;
                while let Some(c) = self.peek() {
                    if brackets == 0 && (c.is_whitespace() || c == ';') {
                        break;
                    }
                    match c {
                        '\\' => self.i += 1,
                        '[' if !quoted => brackets += 1,
                        ']' if !quoted => brackets = brackets.saturating_sub(1),
                        '"' if brackets > 0 => quoted = !quoted,
                        _ => {}
                    }
                    self.i += 1;
                }
                let end = self.i.min(self.c.len());
                Word::Bare(self.c[start..end].iter().collect())
            }
        }
    }
}

/// Tcl's `string match`: `*`, `?`, `[chars]` (with `a-z` ranges) and
/// `\x` for a literal `x`.
pub fn glob(pattern: &str, s: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = s.chars().collect();
    glob_at(&p, &t)
}

fn glob_at(p: &[char], t: &[char]) -> bool {
    let Some((&c, rest)) = p.split_first() else { return t.is_empty() };
    match c {
        '*' => (0..=t.len()).any(|k| glob_at(rest, &t[k..])),
        '?' => !t.is_empty() && glob_at(rest, &t[1..]),
        '[' => {
            let Some(close) = rest.iter().position(|&x| x == ']') else {
                return t.first() == Some(&'[') && glob_at(rest, &t[1..]);
            };
            let set = &rest[..close];
            let Some(&ch) = t.first() else { return false };
            let mut hit = false;
            let mut k = 0;
            while k < set.len() {
                if k + 2 < set.len() && set[k + 1] == '-' {
                    hit |= set[k] <= ch && ch <= set[k + 2];
                    k += 3;
                } else {
                    hit |= set[k] == ch;
                    k += 1;
                }
            }
            hit && glob_at(&rest[close + 1..], &t[1..])
        }
        '\\' if !rest.is_empty() => t.first() == Some(&rest[0]) && glob_at(&rest[1..], &t[1..]),
        _ => t.first() == Some(&c) && glob_at(rest, &t[1..]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_group_braces_and_quotes() {
        let w = list(r#"dg-skip-if "too big" { pdp11-*-* } { "-mint32" } {}"#);
        assert_eq!(
            w,
            [
                Word::Bare("dg-skip-if".into()),
                Word::Quoted("too big".into()),
                Word::Braced(" pdp11-*-* ".into()),
                Word::Braced(r#" "-mint32" "#.into()),
                Word::Braced(String::new()),
            ]
        );
        assert_eq!(list_strings("{ { a b } && ! c }"), [" { a b } && ! c "]);
        assert_eq!(list_strings(" { a b } && ! c "), [" a b ", "&&", "!", "c"]);
        assert_eq!(list(r#"[istarget "x-*-*"] y"#)[0], Word::Bare(r#"[istarget "x-*-*"]"#.into()));
    }

    #[test]
    fn scripts_split_into_commands() {
        let s = "# c\nif [istarget \"avr-*-*\"] {\n    return 1\n}\nset a b; lappend a c \\\n d\nreturn 0\n";
        let c = commands(s);
        assert_eq!(c.len(), 4);
        assert_eq!(c[0][0], Word::Bare("if".into()));
        assert_eq!(c[0][2], Word::Braced("\n    return 1\n".into()));
        assert_eq!(c[1].len(), 3);
        assert_eq!(c[2].len(), 4);
        assert_eq!(c[3], [Word::Bare("return".into()), Word::Bare("0".into())]);
    }

    #[test]
    fn globs() {
        assert!(glob("aarch64*-*-*", "aarch64-unknown-linux-gnu"));
        assert!(glob("*-*-linux*", "x86_64-pc-linux-gnu"));
        assert!(!glob("i?86-*-*", "x86_64-pc-linux-gnu"));
        assert!(glob("i?86-*-*", "i686-pc-linux-gnu"));
        assert!(glob("*-flto*", "-O2 -flto -fno-x"));
        assert!(glob(r"i?86-*-freebsd*\[123\]\.*", "i386-x-freebsd4[123].0"));
        assert!(glob("powerpc[l]e-*", "powerpcle-x"));
        assert!(glob("[a-c]x", "bx") && !glob("[a-c]x", "dx"));
        assert!(glob("*", "") && !glob("?", ""));
    }
}
