//! A small JSON value with a reader and a deterministic pretty-printer, so the
//! harness can keep `bench/` records without a third-party dependency.
//!
//! Objects keep their keys in insertion order, which is what makes the output
//! stable: writers build objects in a fixed order (struct fields in
//! declaration order, maps sorted by key). Numbers are `f64`; integers up to
//! 2^53 round-trip exactly and are printed without a fraction.

use std::fmt::Write as _;

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    /// Key order is preserved; duplicate keys are rejected by the reader.
    Obj(Vec<(String, Json)>),
}

/// Maximum nesting accepted by the reader (the records are 4–5 deep).
const MAX_DEPTH: usize = 64;

impl Json {
    /// Builds an object from `(key, value)` pairs, in the given order.
    pub fn obj<K: Into<String>>(pairs: impl IntoIterator<Item = (K, Json)>) -> Self {
        Self::Obj(pairs.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    pub fn str(s: impl Into<String>) -> Self {
        Self::Str(s.into())
    }

    /// `Some(n)` as a number, `None` as `null`.
    pub fn opt_num(n: Option<f64>) -> Self {
        n.map_or(Self::Null, Self::Num)
    }

    /// The value of `key` if `self` is an object that has it.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Self::Obj(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Num(n) => Some(*n),
            _ => None,
        }
    }

    /// A non-negative integral number that fits in a `u64` exactly.
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            #[allow(clippy::cast_precision_loss)]
            Self::Num(n) if n.fract() == 0.0 && *n >= 0.0 && *n <= (1u64 << 53) as f64 => {
                Some(*n as u64)
            }
            _ => None,
        }
    }

    pub fn as_obj(&self) -> Option<&[(String, Json)]> {
        match self {
            Self::Obj(p) => Some(p),
            _ => None,
        }
    }

    pub fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Self::Arr(a) => Some(a),
            _ => None,
        }
    }

    /// Pretty-prints with two-space indentation and a trailing newline.
    /// Arrays of scalars stay on one line.
    pub fn to_pretty(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0);
        out.push('\n');
        out
    }

    fn write(&self, out: &mut String, indent: usize) {
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Self::Num(n) => write_num(out, *n),
            Self::Str(s) => write_str(out, s),
            Self::Arr(items) if items.iter().all(Json::is_scalar) => {
                out.push('[');
                for (i, v) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    v.write(out, indent);
                }
                out.push(']');
            }
            Self::Arr(items) => {
                out.push('[');
                for (i, v) in items.iter().enumerate() {
                    out.push_str(if i > 0 { ",\n" } else { "\n" });
                    push_indent(out, indent + 1);
                    v.write(out, indent + 1);
                }
                out.push('\n');
                push_indent(out, indent);
                out.push(']');
            }
            Self::Obj(pairs) if pairs.is_empty() => out.push_str("{}"),
            Self::Obj(pairs) => {
                out.push('{');
                for (i, (k, v)) in pairs.iter().enumerate() {
                    out.push_str(if i > 0 { ",\n" } else { "\n" });
                    push_indent(out, indent + 1);
                    write_str(out, k);
                    out.push_str(": ");
                    v.write(out, indent + 1);
                }
                out.push('\n');
                push_indent(out, indent);
                out.push('}');
            }
        }
    }

    fn is_scalar(&self) -> bool {
        !matches!(self, Self::Arr(_) | Self::Obj(_))
    }

    /// Parses a complete JSON document (surrounding whitespace allowed).
    pub fn parse(text: &str) -> Result<Json, String> {
        let mut r = Reader { s: text.as_bytes(), pos: 0 };
        let v = r.value(0)?;
        r.ws();
        if r.pos != r.s.len() {
            return Err(r.error("trailing characters after the JSON value"));
        }
        Ok(v)
    }
}

fn push_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push_str("  ");
    }
}

fn write_num(out: &mut String, n: f64) {
    if !n.is_finite() {
        out.push_str("null");
    } else if n.fract() == 0.0 && n.abs() < 1e15 {
        let _ = write!(out, "{}", n as i64);
    } else {
        // `Display` for f64 is the shortest text that round-trips.
        let _ = write!(out, "{n}");
    }
}

fn write_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

struct Reader<'a> {
    s: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn error(&self, msg: &str) -> String {
        format!("JSON error at byte {}: {msg}", self.pos)
    }

    fn ws(&mut self) {
        while self.s.get(self.pos).is_some_and(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, lit: &str) -> bool {
        if self.s[self.pos..].starts_with(lit.as_bytes()) {
            self.pos += lit.len();
            true
        } else {
            false
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > MAX_DEPTH {
            return Err(self.error("nesting too deep"));
        }
        self.ws();
        match self.s.get(self.pos) {
            None => Err(self.error("unexpected end of input")),
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => self.string().map(Json::Str),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) if self.eat("null") => Ok(Json::Null),
            Some(_) if self.eat("true") => Ok(Json::Bool(true)),
            Some(_) if self.eat("false") => Ok(Json::Bool(false)),
            Some(_) => Err(self.error("expected a JSON value")),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, String> {
        self.pos += 1; // {
        let mut pairs: Vec<(String, Json)> = Vec::new();
        self.ws();
        if self.eat("}") {
            return Ok(Json::Obj(pairs));
        }
        loop {
            self.ws();
            if self.s.get(self.pos) != Some(&b'"') {
                return Err(self.error("expected an object key"));
            }
            let key = self.string()?;
            if pairs.iter().any(|(k, _)| *k == key) {
                return Err(self.error(&format!("duplicate key {key:?}")));
            }
            self.ws();
            if !self.eat(":") {
                return Err(self.error("expected `:`"));
            }
            let v = self.value(depth + 1)?;
            pairs.push((key, v));
            self.ws();
            if self.eat("}") {
                return Ok(Json::Obj(pairs));
            }
            if !self.eat(",") {
                return Err(self.error("expected `,` or `}`"));
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Json, String> {
        self.pos += 1; // [
        let mut items = Vec::new();
        self.ws();
        if self.eat("]") {
            return Ok(Json::Arr(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.ws();
            if self.eat("]") {
                return Ok(Json::Arr(items));
            }
            if !self.eat(",") {
                return Err(self.error("expected `,` or `]`"));
            }
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        while self
            .s
            .get(self.pos)
            .is_some_and(|b| matches!(b, b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'))
        {
            self.pos += 1;
        }
        let text = std::str::from_utf8(&self.s[start..self.pos]).unwrap_or_default();
        // Rust's float grammar is a superset of JSON's except for these.
        let bad = text.starts_with('+')
            || text.starts_with('.')
            || text.starts_with("-.")
            || text.ends_with('.')
            || text.contains(".e")
            || text.contains(".E")
            || {
                let digits = text.trim_start_matches('-').as_bytes();
                digits.len() > 1 && digits[0] == b'0' && digits[1].is_ascii_digit()
            };
        match text.parse::<f64>() {
            Ok(n) if !bad && n.is_finite() => Ok(Json::Num(n)),
            _ => {
                self.pos = start;
                Err(self.error(&format!("bad number `{text}`")))
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self.s.get(self.pos..self.pos + 4).ok_or_else(|| self.error("bad \\u"))?;
        let v = std::str::from_utf8(digits)
            .ok()
            .and_then(|d| u32::from_str_radix(d, 16).ok())
            .ok_or_else(|| self.error("bad \\u escape"))?;
        self.pos += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, String> {
        self.pos += 1; // "
        let mut out = Vec::new();
        loop {
            let Some(&b) = self.s.get(self.pos) else {
                return Err(self.error("unterminated string"));
            };
            self.pos += 1;
            match b {
                b'"' => break,
                b'\\' => {
                    let Some(&e) = self.s.get(self.pos) else {
                        return Err(self.error("unterminated string"));
                    };
                    self.pos += 1;
                    let c = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let hi = self.hex4()?;
                            let code = if (0xD800..0xDC00).contains(&hi) {
                                if !self.eat("\\u") {
                                    return Err(self.error("unpaired surrogate"));
                                }
                                let lo = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return Err(self.error("unpaired surrogate"));
                                }
                                0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                            } else {
                                hi
                            };
                            char::from_u32(code).ok_or_else(|| self.error("bad \\u escape"))?
                        }
                        _ => return Err(self.error("unknown escape")),
                    };
                    let mut buf = [0; 4];
                    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                }
                b if b < 0x20 => return Err(self.error("control character in string")),
                b => out.push(b),
            }
        }
        String::from_utf8(out).map_err(|_| self.error("invalid UTF-8 in string"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_order_and_values() {
        let v = Json::obj([
            ("schema", Json::Num(1.0)),
            ("zeta", Json::str("tab\there \"q\" \\ \u{1} é 🎉")),
            ("alpha", Json::Arr(vec![Json::Num(1.5), Json::Num(-2.0), Json::Num(712.345)])),
            ("nested", Json::obj([("ok", Json::Bool(true)), ("none", Json::Null)])),
            ("empty", Json::obj(Vec::<(String, Json)>::new())),
            ("list", Json::Arr(vec![Json::obj([("a", Json::Num(0.001))])])),
            ("big", Json::Num(9_007_199_254_740_992.0)),
        ]);
        let text = v.to_pretty();
        assert_eq!(Json::parse(&text).unwrap(), v);
        // Stable: printing the re-read value gives byte-identical text.
        assert_eq!(Json::parse(&text).unwrap().to_pretty(), text);
        assert!(text.starts_with("{\n  \"schema\": 1,\n  \"zeta\": "), "{text}");
        assert!(text.contains("\"alpha\": [1.5, -2, 712.345]"), "{text}");
        assert!(text.contains("\"empty\": {}"), "{text}");
        assert!(text.contains("\\u0001"), "{text}");
    }

    #[test]
    fn reads_foreign_json() {
        let v =
            Json::parse(r#" {"a": [1e3, -0.5E-1, 0], "b": "\u00e9\ud83c\udf89\/", "c": false} "#)
                .unwrap();
        assert_eq!(v.get("a").unwrap().as_arr().unwrap()[0].as_u64(), Some(1000));
        assert_eq!(v.get("a").unwrap().as_arr().unwrap()[1].as_f64(), Some(-0.05));
        assert_eq!(v.get("b").unwrap().as_str(), Some("é🎉/"));
        assert_eq!(v.get("c"), Some(&Json::Bool(false)));
        assert_eq!(v.get("missing"), None);
        assert_eq!(Json::Num(-1.0).as_u64(), None);
        assert_eq!(Json::Num(1.5).as_u64(), None);
    }

    #[test]
    fn rejects_malformed_json() {
        for bad in [
            "",
            "{",
            "{\"a\" 1}",
            "{\"a\": 1,}",
            "[1 2]",
            "\"open",
            "{\"a\": 1, \"a\": 2}",
            "01",
            "-00.5",
            "+1",
            ".5",
            "1.",
            "nul",
            "\"\\x\"",
            "\"\\ud800\"",
            "[1] [2]",
            "\"a\nb\"",
        ] {
            assert!(Json::parse(bad).is_err(), "{bad:?} should be rejected");
        }
        let deep = "[".repeat(100) + &"]".repeat(100);
        assert!(Json::parse(&deep).unwrap_err().contains("too deep"));
    }

    #[test]
    fn non_finite_numbers_print_as_null() {
        assert_eq!(Json::Num(f64::NAN).to_pretty(), "null\n");
        assert_eq!(Json::opt_num(None).to_pretty(), "null\n");
    }
}
