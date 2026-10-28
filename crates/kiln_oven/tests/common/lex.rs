//! The lexer of the test DSL: identifiers, numbers, character and string
//! literals (with their encoding prefixes) and C++ punctuators.

/// A token kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tok {
    /// An identifier or keyword.
    Ident(String),
    /// A pp-number, as spelled.
    Num(String),
    /// A character literal, as spelled (with its prefix).
    Char(String),
    /// A string literal, as spelled (with its prefix).
    Str(String),
    /// A punctuator.
    Punct(&'static str),
    /// The end of the input.
    Eof,
}

/// A token and its byte offset.
#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    pub at: usize,
}

const PUNCTS: &[&str] = &[
    "<<=", ">>=", "...", "->", "++", "--", "<<", ">>", "<=", ">=", "==", "!=", "&&", "||", "+=",
    "-=", "*=", "/=", "%=", "&=", "^=", "|=", "::", "+", "-", "*", "/", "%", "<", ">", "=", "!",
    "&", "|", "^", "~", "?", ":", ";", ",", ".", "(", ")", "[", "]", "{", "}",
];

/// Splits `src` into tokens.
///
/// # Panics
/// On a character that starts no token or an unterminated literal.
pub fn lex(src: &str) -> Vec<Token> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if src[i..].starts_with("//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if src[i..].starts_with("/*") {
            let end = src[i + 2..].find("*/").expect("unterminated comment");
            i += end + 4;
            continue;
        }
        let at = i;
        if c.is_ascii_alphabetic() || c == b'_' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let word = &src[at..i];
            if i < b.len()
                && (b[i] == b'\'' || b[i] == b'"')
                && matches!(word, "u8" | "u" | "U" | "L")
            {
                let q = b[i];
                i = skip_quoted(b, i);
                let s = src[at..i].to_owned();
                out.push(Token { tok: if q == b'\'' { Tok::Char(s) } else { Tok::Str(s) }, at });
                continue;
            }
            out.push(Token { tok: Tok::Ident(word.to_owned()), at });
            continue;
        }
        if c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit)) {
            i += 1;
            while i < b.len() {
                let d = b[i];
                let exp = matches!(b[i - 1], b'e' | b'E' | b'p' | b'P')
                    && !src[at..i].starts_with("0x")
                    || matches!(b[i - 1], b'p' | b'P');
                if d.is_ascii_alphanumeric()
                    || d == b'.'
                    || d == b'\''
                    || (exp && (d == b'+' || d == b'-'))
                {
                    i += 1;
                } else {
                    break;
                }
            }
            out.push(Token { tok: Tok::Num(src[at..i].replace('\'', "")), at });
            continue;
        }
        if c == b'\'' || c == b'"' {
            i = skip_quoted(b, i);
            let s = src[at..i].to_owned();
            out.push(Token { tok: if c == b'\'' { Tok::Char(s) } else { Tok::Str(s) }, at });
            continue;
        }
        let p = PUNCTS
            .iter()
            .find(|p| src[i..].starts_with(**p))
            .unwrap_or_else(|| panic!("unexpected character {:?} at offset {i}", c as char));
        i += p.len();
        out.push(Token { tok: Tok::Punct(p), at });
    }
    out.push(Token { tok: Tok::Eof, at: b.len() });
    out
}

fn skip_quoted(b: &[u8], mut i: usize) -> usize {
    let q = b[i];
    i += 1;
    while i < b.len() && b[i] != q {
        if b[i] == b'\\' {
            i += 1;
        }
        i += 1;
    }
    assert!(i < b.len(), "unterminated literal");
    i + 1
}

/// Decodes the escape sequences of a literal body into code points.
pub fn decode(body: &str) -> Vec<u32> {
    let b: Vec<char> = body.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] != '\\' {
            out.push(b[i] as u32);
            i += 1;
            continue;
        }
        i += 1;
        let c = b[i];
        i += 1;
        let v = match c {
            'n' => 10,
            't' => 9,
            'r' => 13,
            'a' => 7,
            'b' => 8,
            'f' => 12,
            'v' => 11,
            'x' => {
                let mut v = 0u32;
                while i < b.len() && b[i].is_ascii_hexdigit() {
                    v = v * 16 + b[i].to_digit(16).expect("hex");
                    i += 1;
                }
                v
            }
            '0'..='7' => {
                let mut v = c.to_digit(8).expect("oct");
                let mut n = 1;
                while n < 3 && i < b.len() && b[i].is_digit(8) {
                    v = v * 8 + b[i].to_digit(8).expect("oct");
                    i += 1;
                    n += 1;
                }
                v
            }
            other => other as u32,
        };
        out.push(v);
    }
    out
}
