//! The regular expressions of `-verify`'s `-re` directives: POSIX
//! extended regular expressions as clang's `llvm::Regex` (Henry Spencer's
//! `regcomp`) accepts them, matched by an unanchored search. Hand-written
//! because kiln takes no third-party dependency for it (spec/diag.md
//! §11.4). Byte-oriented, like `llvm::Regex`; matched with a Pike VM (no
//! back-references in ERE), so matching is linear in the subject.
//!
//! Supported: literals, `\c` (any character `c`, literally), `.`,
//! bracket expressions (`[abc]`, `[^a-z]`, `[[:alpha:]]`, `[[.c.]]`,
//! `[[=c=]]`), `^`, `$`, `( )`, `|`, and one of `*`, `+`, `?`, `{m}`,
//! `{m,}`, `{m,n}` (counts up to 255) after an atom. Errors carry
//! `llvm::regerror`'s wording.

/// An `llvm::regerror` message.
type Error = &'static str;

const EESCAPE: Error = "trailing backslash (\\)";
const EBRACK: Error = "brackets ([ ]) not balanced";
const EPAREN: Error = "parentheses not balanced";
const EBRACE: Error = "braces not balanced";
const BADBR: Error = "invalid repetition count(s)";
const ERANGE: Error = "invalid character range";
const ECTYPE: Error = "invalid character class";
const ECOLLATE: Error = "invalid collating element";
const BADRPT: Error = "repetition-operator operand invalid";
const EMPTY: Error = "empty (sub)expression";
const ESPACE: Error = "out of memory";

/// Spencer's `DUPMAX`.
const DUP_MAX: u32 = 255;
/// Program size past which compilation fails with `ESPACE` (nested
/// bounds multiply).
const MAX_INSTS: usize = 1 << 16;

/// A set of bytes.
#[derive(Clone, PartialEq, Eq, Debug)]
struct ByteSet([u64; 4]);

impl ByteSet {
    fn new() -> Self {
        ByteSet([0; 4])
    }
    fn insert(&mut self, b: u8) {
        self.0[usize::from(b >> 6)] |= 1 << (b & 63);
    }
    fn contains(&self, b: u8) -> bool {
        self.0[usize::from(b >> 6)] & (1 << (b & 63)) != 0
    }
    fn negate(&mut self) {
        for w in &mut self.0 {
            *w = !*w;
        }
    }
}

#[derive(Clone, Debug)]
enum Node {
    Set(ByteSet),
    Any,
    Bol,
    Eol,
    Concat(Vec<Node>),
    Alt(Vec<Node>),
    Repeat(Box<Node>, u32, Option<u32>),
}

#[derive(Clone, Debug)]
enum Inst {
    Set(usize),
    Any,
    Bol,
    Eol,
    Split(usize, usize),
    Jmp(usize),
    Match,
}

/// A compiled regular expression.
#[derive(Clone, Debug)]
pub struct Regex {
    insts: Vec<Inst>,
    sets: Vec<ByteSet>,
}

impl Regex {
    /// Compiles `pattern`, or returns `llvm::regerror`'s message.
    pub fn new(pattern: &str) -> Result<Regex, &'static str> {
        let mut p = Parser { s: pattern.as_bytes(), i: 0 };
        let node = p.ere(None)?;
        if p.i < p.s.len() {
            // Only an unmatched `)` stops the top level early.
            return Err(EPAREN);
        }
        let mut c = Compiler { insts: Vec::new(), sets: Vec::new() };
        c.emit(&node)?;
        c.push(Inst::Match)?;
        Ok(Regex { insts: c.insts, sets: c.sets })
    }

    /// Whether the expression matches somewhere in `subject`.
    pub fn is_match(&self, subject: &str) -> bool {
        let s = subject.as_bytes();
        let n = self.insts.len();
        let mut cur = Threads::new(n);
        let mut next = Threads::new(n);
        let mut stack = Vec::new();
        for pos in 0..=s.len() {
            // Unanchored: a thread starts at every position.
            if self.add(&mut cur, &mut stack, 0, pos, s.len()) {
                return true;
            }
            let Some(&b) = s.get(pos) else { break };
            next.clear();
            for k in 0..cur.len {
                let pc = cur.dense[k];
                let ok = match &self.insts[pc] {
                    Inst::Set(i) => self.sets[*i].contains(b),
                    Inst::Any => true,
                    _ => false,
                };
                if ok && self.add(&mut next, &mut stack, pc + 1, pos + 1, s.len()) {
                    return true;
                }
            }
            std::mem::swap(&mut cur, &mut next);
        }
        false
    }

    /// Adds the thread at `pc` (following empty transitions) to `t`;
    /// true if it reaches `Match`.
    fn add(
        &self,
        t: &mut Threads,
        stack: &mut Vec<usize>,
        pc: usize,
        pos: usize,
        len: usize,
    ) -> bool {
        stack.push(pc);
        while let Some(pc) = stack.pop() {
            if !t.insert(pc) {
                continue;
            }
            match self.insts[pc] {
                Inst::Match => {
                    stack.clear();
                    return true;
                }
                Inst::Jmp(x) => stack.push(x),
                Inst::Split(a, b) => {
                    stack.push(b);
                    stack.push(a);
                }
                Inst::Bol => {
                    if pos == 0 {
                        stack.push(pc + 1);
                    }
                }
                Inst::Eol => {
                    if pos == len {
                        stack.push(pc + 1);
                    }
                }
                Inst::Set(_) | Inst::Any => {}
            }
        }
        false
    }
}

/// A sparse set of program counters (Pike VM thread list).
struct Threads {
    dense: Vec<usize>,
    sparse: Vec<usize>,
    len: usize,
}

impl Threads {
    fn new(n: usize) -> Self {
        Threads { dense: vec![0; n], sparse: vec![0; n], len: 0 }
    }
    fn clear(&mut self) {
        self.len = 0;
    }
    fn insert(&mut self, pc: usize) -> bool {
        let i = self.sparse[pc];
        if i < self.len && self.dense[i] == pc {
            return false;
        }
        self.sparse[pc] = self.len;
        self.dense[self.len] = pc;
        self.len += 1;
        true
    }
}

struct Parser<'s> {
    s: &'s [u8],
    i: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    fn peek2(&self) -> Option<u8> {
        self.s.get(self.i + 1).copied()
    }
    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    /// Spencer `p_ere`: branches up to `stop` (`)`) or the end.
    fn ere(&mut self, stop: Option<u8>) -> Result<Node, Error> {
        let mut branches = Vec::new();
        loop {
            let mut items = Vec::new();
            while let Some(c) = self.peek() {
                if c == b'|' || Some(c) == stop {
                    break;
                }
                items.push(self.ere_exp()?);
            }
            if items.is_empty() {
                return Err(EMPTY);
            }
            branches.push(Node::Concat(items));
            if !self.eat(b'|') {
                break;
            }
        }
        Ok(if branches.len() == 1 {
            branches.pop().unwrap_or(Node::Concat(Vec::new()))
        } else {
            Node::Alt(branches)
        })
    }

    /// Spencer `p_ere_exp`: one atom and at most one repetition.
    fn ere_exp(&mut self) -> Result<Node, Error> {
        let Some(c) = self.peek() else { return Err(EMPTY) };
        self.i += 1;
        let mut was_caret = false;
        let atom = match c {
            b'(' => {
                if self.peek().is_none() {
                    return Err(EPAREN);
                }
                let inner = if self.peek() == Some(b')') {
                    Node::Concat(Vec::new())
                } else {
                    self.ere(Some(b')'))?
                };
                if !self.eat(b')') {
                    return Err(EPAREN);
                }
                inner
            }
            b')' => return Err(EPAREN),
            b'^' => {
                was_caret = true;
                Node::Bol
            }
            b'$' => Node::Eol,
            b'*' | b'+' | b'?' => return Err(BADRPT),
            b'{' if self.peek().is_some_and(|d| d.is_ascii_digit()) => return Err(BADRPT),
            b'.' => Node::Any,
            b'[' => self.bracket()?,
            b'\\' => {
                let Some(e) = self.peek() else { return Err(EESCAPE) };
                self.i += 1;
                literal(e)
            }
            _ => literal(c),
        };
        let Some(c) = self.peek() else { return Ok(atom) };
        let is_bound = c == b'{' && self.peek2().is_some_and(|d| d.is_ascii_digit());
        if !(c == b'*' || c == b'+' || c == b'?' || is_bound) {
            return Ok(atom);
        }
        if was_caret {
            return Err(BADRPT);
        }
        self.i += 1;
        let (min, max) = match c {
            b'*' => (0, None),
            b'+' => (1, None),
            b'?' => (0, Some(1)),
            _ => {
                let min = self.count()?;
                let max = if self.eat(b',') {
                    if self.peek().is_some_and(|d| d.is_ascii_digit()) {
                        let m = self.count()?;
                        if m < min {
                            return Err(BADBR);
                        }
                        Some(m)
                    } else {
                        None
                    }
                } else {
                    Some(min)
                };
                if !self.eat(b'}') {
                    while self.peek().is_some_and(|c| c != b'}') {
                        self.i += 1;
                    }
                    return Err(if self.peek().is_none() { EBRACE } else { BADBR });
                }
                (min, max)
            }
        };
        if let Some(c) = self.peek() {
            let is_bound = c == b'{' && self.peek2().is_some_and(|d| d.is_ascii_digit());
            if c == b'*' || c == b'+' || c == b'?' || is_bound {
                return Err(BADRPT);
            }
        }
        Ok(Node::Repeat(Box::new(atom), min, max))
    }

    /// Spencer `p_count`.
    fn count(&mut self) -> Result<u32, Error> {
        let mut n: u32 = 0;
        let mut digits = 0;
        while let Some(d) = self.peek().filter(u8::is_ascii_digit) {
            if n <= DUP_MAX {
                n = n * 10 + u32::from(d - b'0');
            }
            digits += 1;
            self.i += 1;
        }
        if digits == 0 || n > DUP_MAX {
            return Err(BADBR);
        }
        Ok(n)
    }

    /// Spencer `p_bracket` (after the `[`).
    fn bracket(&mut self) -> Result<Node, Error> {
        let mut set = ByteSet::new();
        let negate = self.eat(b'^');
        if self.eat(b']') {
            set.insert(b']');
        } else if self.eat(b'-') {
            set.insert(b'-');
        }
        while let Some(c) = self.peek() {
            if c == b']' || (c == b'-' && self.peek2() == Some(b']')) {
                break;
            }
            self.b_term(&mut set)?;
        }
        if self.eat(b'-') {
            set.insert(b'-');
        }
        if !self.eat(b']') {
            return Err(EBRACK);
        }
        if negate {
            set.negate();
        }
        Ok(Node::Set(set))
    }

    /// Spencer `p_b_term`: one term of a bracket expression.
    fn b_term(&mut self, set: &mut ByteSet) -> Result<(), Error> {
        let c = self.peek();
        let c2 = self.peek2();
        if c == Some(b'[') && matches!(c2, Some(b':' | b'=')) {
            self.i += 2;
            let delim = c2.unwrap_or(b':');
            let start = self.i;
            while self.peek().is_some_and(|c| c != delim) {
                self.i += 1;
            }
            if self.peek().is_none() {
                return Err(EBRACK);
            }
            let name = &self.s[start..self.i];
            if !(self.eat(delim) && self.eat(b']')) {
                return Err(if delim == b':' { ECTYPE } else { ECOLLATE });
            }
            if delim == b':' {
                return class(name, set);
            }
            // An equivalence class is its collating element.
            set.insert(collating(name)?);
            return Ok(());
        }
        if c == Some(b'-') {
            return Err(ERANGE);
        }
        let start = self.b_symbol()?;
        if self.peek() == Some(b'-') && self.peek2() != Some(b']') {
            self.i += 1;
            let finish = if self.eat(b'-') { b'-' } else { self.b_symbol()? };
            if start > finish {
                return Err(ERANGE);
            }
            for b in start..=finish {
                set.insert(b);
            }
        } else {
            set.insert(start);
        }
        Ok(())
    }

    /// Spencer `p_b_symbol`: a character or `[.name.]`.
    fn b_symbol(&mut self) -> Result<u8, Error> {
        let Some(c) = self.peek() else { return Err(EBRACK) };
        if c == b'[' && self.peek2() == Some(b'.') {
            self.i += 2;
            let start = self.i;
            while self.peek().is_some_and(|c| c != b'.') {
                self.i += 1;
            }
            if self.peek().is_none() {
                return Err(EBRACK);
            }
            let name = &self.s[start..self.i];
            if !(self.eat(b'.') && self.eat(b']')) {
                return Err(ECOLLATE);
            }
            return collating(name);
        }
        self.i += 1;
        Ok(c)
    }
}

fn literal(b: u8) -> Node {
    let mut s = ByteSet::new();
    s.insert(b);
    Node::Set(s)
}

/// A collating element: a single character (Spencer's named elements
/// like `space` are not needed by any test).
fn collating(name: &[u8]) -> Result<u8, Error> {
    match name {
        [c] => Ok(*c),
        _ => Err(ECOLLATE),
    }
}

/// A `[:name:]` character class (the "C" locale).
fn class(name: &[u8], set: &mut ByteSet) -> Result<(), Error> {
    let pred: fn(u8) -> bool = match name {
        b"alnum" => |b| b.is_ascii_alphanumeric(),
        b"alpha" => |b| b.is_ascii_alphabetic(),
        b"blank" => |b| b == b' ' || b == b'\t',
        b"cntrl" => |b| b.is_ascii_control(),
        b"digit" => |b| b.is_ascii_digit(),
        b"graph" => |b| b.is_ascii_graphic(),
        b"lower" => |b| b.is_ascii_lowercase(),
        b"print" => |b| b.is_ascii_graphic() || b == b' ',
        b"punct" => |b| b.is_ascii_punctuation(),
        b"space" => |b| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c),
        b"upper" => |b| b.is_ascii_uppercase(),
        b"xdigit" => |b| b.is_ascii_hexdigit(),
        _ => return Err(ECTYPE),
    };
    for b in 0..=255u8 {
        if pred(b) {
            set.insert(b);
        }
    }
    Ok(())
}

struct Compiler {
    insts: Vec<Inst>,
    sets: Vec<ByteSet>,
}

impl Compiler {
    fn push(&mut self, i: Inst) -> Result<usize, Error> {
        if self.insts.len() >= MAX_INSTS {
            return Err(ESPACE);
        }
        self.insts.push(i);
        Ok(self.insts.len() - 1)
    }

    fn emit(&mut self, n: &Node) -> Result<(), Error> {
        match n {
            Node::Set(s) => {
                self.sets.push(s.clone());
                self.push(Inst::Set(self.sets.len() - 1))?;
            }
            Node::Any => {
                self.push(Inst::Any)?;
            }
            Node::Bol => {
                self.push(Inst::Bol)?;
            }
            Node::Eol => {
                self.push(Inst::Eol)?;
            }
            Node::Concat(items) => {
                for i in items {
                    self.emit(i)?;
                }
            }
            Node::Alt(branches) => {
                let mut jumps = Vec::new();
                for (k, b) in branches.iter().enumerate() {
                    if k + 1 < branches.len() {
                        let split = self.push(Inst::Split(0, 0))?;
                        self.emit(b)?;
                        jumps.push(self.push(Inst::Jmp(0))?);
                        self.insts[split] = Inst::Split(split + 1, self.insts.len());
                    } else {
                        self.emit(b)?;
                    }
                }
                let end = self.insts.len();
                for j in jumps {
                    self.insts[j] = Inst::Jmp(end);
                }
            }
            Node::Repeat(body, min, max) => {
                for _ in 0..*min {
                    self.emit(body)?;
                }
                match max {
                    None => {
                        let split = self.push(Inst::Split(0, 0))?;
                        self.emit(body)?;
                        self.push(Inst::Jmp(split))?;
                        self.insts[split] = Inst::Split(split + 1, self.insts.len());
                    }
                    Some(max) => {
                        let mut splits = Vec::new();
                        for _ in *min..*max {
                            splits.push(self.push(Inst::Split(0, 0))?);
                            self.emit(body)?;
                        }
                        let end = self.insts.len();
                        for s in splits {
                            self.insts[s] = Inst::Split(s + 1, end);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// `llvm::Regex::escape`: `s` with every ERE metacharacter
/// (`()^$|*+?.[]\{}`) backslash-escaped.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "()^$|*+?.[]\\{}".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(p: &str, s: &str) -> bool {
        Regex::new(p).unwrap_or_else(|e| panic!("{p}: {e}")).is_match(s)
    }

    #[test]
    fn literals_and_search() {
        assert!(m("abc", "xxabcxx"));
        assert!(!m("abd", "xxabcxx"));
        assert!(m("a\\.c", "a.c"));
        assert!(!m("a\\.c", "abc"));
        assert!(m("a.c", "abc"));
        assert!(m("\\{\\}", "{}"));
    }

    #[test]
    fn anchors() {
        assert!(m("^ab", "abc"));
        assert!(!m("^bc", "abc"));
        assert!(m("bc$", "abc"));
        assert!(!m("ab$", "abc"));
        assert!(m("^$", ""));
    }

    #[test]
    fn repetition_and_groups() {
        assert!(m("^a*$", ""));
        assert!(m("^a+b?$", "aaa"));
        assert!(!m("^a+$", ""));
        assert!(m("^(ab)+$", "ababab"));
        assert!(m("^a{2,3}$", "aaa"));
        assert!(!m("^a{2,3}$", "aaaa"));
        assert!(m("^a{2}$", "aa"));
        assert!(m("^a{2,}$", "aaaaa"));
        assert!(m("^(a*)*$", "aaa"));
        assert!(m("^()$", ""));
        assert!(m("x{", "x{"));
    }

    #[test]
    fn alternation() {
        assert!(m("^(static_assert|static assertion) failed", "static assertion failed"));
        assert!(m("cat|dog", "hotdog"));
        assert!(!m("^(cat|dog)$", "cow"));
    }

    #[test]
    fn brackets() {
        assert!(m("^[abc]+$", "cab"));
        assert!(m("^[^abc]$", "d"));
        assert!(!m("^[^abc]$", "a"));
        assert!(m("^[a-z0-9_]+$", "ab_9"));
        assert!(m("^[]a]+$", "]a"));
        assert!(m("^[a-]+$", "-a"));
        assert!(m("^[[:alpha:][:digit:]]+$", "a1"));
        assert!(m("[[:space:]]", "a b"));
        assert!(m("^[[.-.]]$", "-"));
        assert!(m("^[\\]$", "\\"));
    }

    #[test]
    fn long_subject_is_linear() {
        let s = "a".repeat(5000);
        assert!(!m("(.*)(.*)(.*)(.*)b", &s));
    }

    #[test]
    fn errors_use_llvm_wording() {
        let e = |p: &str| Regex::new(p).err();
        assert_eq!(e("("), Some(EPAREN));
        assert_eq!(e("a)"), Some(EPAREN));
        assert_eq!(e("[a"), Some(EBRACK));
        assert_eq!(e("*a"), Some(BADRPT));
        assert_eq!(e("a**"), Some(BADRPT));
        assert_eq!(e("a|"), Some(EMPTY));
        assert_eq!(e("a\\"), Some(EESCAPE));
        assert_eq!(e("a{3,2}"), Some(BADBR));
        assert_eq!(e("a{1"), Some(EBRACE));
        assert_eq!(e("a{1x}"), Some(BADBR));
        assert_eq!(e("[z-a]"), Some(ERANGE));
        assert_eq!(e("[[:foo:]]"), Some(ECTYPE));
        assert_eq!(e("a{256}"), Some(BADBR));
        assert_eq!(e("^*"), Some(BADRPT));
    }

    #[test]
    fn escape_metachars() {
        assert_eq!(
            escape("a(b)*[c]{d}.$^|+?\\"),
            "a\\(b\\)\\*\\[c\\]\\{d\\}\\.\\$\\^\\|\\+\\?\\\\"
        );
        assert!(m(&escape("f(int) [T = int]"), "in f(int) [T = int] here"));
    }
}
