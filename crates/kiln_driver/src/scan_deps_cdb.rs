//! The JSON compilation database of `kiln-scan-deps
//! -compilation-database <file>` (spec/modules.md §11.9), read as clang's
//! `JSONCompilationDatabase` reads it: a top-level array of objects with
//! the keys `directory`, `file`, `command` or `arguments`, and an
//! optional `output`; `arguments` wins over `command`, which is split as
//! a POSIX shell would (double quotes with backslash escapes, literal
//! single quotes, backslash escapes outside quotes). Trailing commas are
//! tolerated and scalars of any type are taken by their text, as clang's
//! YAML-based reader does.

/// One entry of the database.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    /// `directory`: the working directory of the command.
    pub directory: String,
    /// `file`: the main source file, as written (the name the scanner
    /// reports the entry by).
    pub file: String,
    /// `arguments`, or `command` split into words; `[0]` is the compiler.
    pub command: Vec<String>,
    /// `output`, if present.
    pub output: Option<String>,
}

/// A parsed JSON value. Scalars keep their text (`3`, `true`, `null`);
/// objects keep their keys in order, duplicates included.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Value {
    Scalar(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

/// Reads the database at `path`; `Err` is the message to print (clang's
/// wording for the shape errors).
pub(crate) fn load(path: &str) -> Result<Vec<Entry>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        let msg = e.to_string();
        let msg = msg.split(" (os error").next().unwrap_or(&msg).to_owned();
        format!("Error while opening JSON database: {msg}")
    })?;
    parse(&text).map_err(|e| match e {
        Error::Syntax { line, col, what } => format!("{path}:{line}:{col}: error: {what}"),
        Error::Shape(msg) => msg,
    })
}

/// Why a database was rejected.
#[derive(Debug, PartialEq, Eq)]
enum Error {
    /// Not JSON: the 1-based position and the problem.
    Syntax { line: usize, col: usize, what: &'static str },
    /// JSON of the wrong shape: clang's message.
    Shape(String),
}

fn shape(msg: &str) -> Error {
    Error::Shape(msg.to_owned())
}

/// Parses a database text.
fn parse(text: &str) -> Result<Vec<Entry>, Error> {
    let mut p = Parser { s: text.as_bytes(), pos: 0 };
    p.skip_ws();
    if p.pos == p.s.len() || p.s[p.pos] != b'[' {
        // An empty file, an object or a scalar: clang's first check.
        return Err(shape("Expected array."));
    }
    let root = p.value()?;
    p.skip_ws();
    if p.pos != p.s.len() {
        return Err(p.err("unexpected text after the database"));
    }
    let Value::Array(items) = root else {
        return Err(shape("Expected array."));
    };
    items.iter().map(entry).collect()
}

/// One object of the top-level array.
fn entry(v: &Value) -> Result<Entry, Error> {
    let Value::Object(fields) = v else {
        return Err(shape("Expected object."));
    };
    let mut directory = None;
    let mut file = None;
    let mut command = None;
    let mut arguments = None;
    let mut output = None;
    for (key, val) in fields {
        let slot = match key.as_str() {
            "directory" => &mut directory,
            "file" => &mut file,
            "command" => &mut command,
            "output" => &mut output,
            "arguments" => {
                let Value::Array(words) = val else {
                    return Err(shape("Expected sequence as value."));
                };
                let words = words.iter().map(scalar).collect::<Result<Vec<_>, _>>()?;
                arguments.get_or_insert(words);
                continue;
            }
            _ => return Err(Error::Shape(format!("Unknown key: \"\"{key}\"\""))),
        };
        let s = scalar(val)?;
        slot.get_or_insert(s);
    }
    let Some(file) = file else {
        return Err(shape("Missing key: \"file\"."));
    };
    let command = match (arguments, command) {
        (Some(a), _) => a,
        (None, Some(c)) => split_command(&c),
        (None, None) => return Err(shape("Missing key: \"command\" or \"arguments\".")),
    };
    let Some(directory) = directory else {
        return Err(shape("Missing key: \"directory\"."));
    };
    Ok(Entry { directory, file, command, output })
}

fn scalar(v: &Value) -> Result<String, Error> {
    match v {
        Value::Scalar(s) => Ok(s.clone()),
        _ => Err(shape("Expected string as value.")),
    }
}

/// Splits a `command` string as clang's `unescapeCommandLine` does
/// (POSIX rules): words separated by blanks; `"..."` with `\` escaping
/// any character; `'...'` literal; `\` outside quotes escaping the next
/// character.
pub(crate) fn split_command(cmd: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut chars = cmd.chars().peekable();
    loop {
        while chars.peek().is_some_and(|c| c.is_ascii_whitespace()) {
            chars.next();
        }
        if chars.peek().is_none() {
            return words;
        }
        let mut word = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_whitespace() {
                break;
            }
            chars.next();
            match c {
                '"' => {
                    while let Some(c) = chars.next() {
                        match c {
                            '"' => break,
                            '\\' => word.extend(chars.next()),
                            c => word.push(c),
                        }
                    }
                }
                '\'' => {
                    for c in chars.by_ref() {
                        if c == '\'' {
                            break;
                        }
                        word.push(c);
                    }
                }
                '\\' => word.extend(chars.next()),
                c => word.push(c),
            }
        }
        words.push(word);
    }
}

/// A small recursive-descent JSON reader (the driver has no JSON
/// dependency).
struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
}

/// Nesting deeper than this is rejected rather than recursed into.
const MAX_DEPTH: usize = 64;

impl Parser<'_> {
    fn err(&self, what: &'static str) -> Error {
        let before = &self.s[..self.pos.min(self.s.len())];
        let line = before.iter().filter(|&&b| b == b'\n').count() + 1;
        let start = before.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
        Error::Syntax { line, col: self.pos - start + 1, what }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.s.len() && matches!(self.s[self.pos], b' ' | b'\t' | b'\n' | b'\r') {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.pos).copied()
    }

    fn value(&mut self) -> Result<Value, Error> {
        self.value_at(0)
    }

    fn value_at(&mut self, depth: usize) -> Result<Value, Error> {
        if depth > MAX_DEPTH {
            return Err(self.err("nesting too deep"));
        }
        self.skip_ws();
        match self.peek() {
            Some(b'[') => {
                self.pos += 1;
                let mut items = Vec::new();
                loop {
                    self.skip_ws();
                    if self.peek() == Some(b']') {
                        self.pos += 1;
                        return Ok(Value::Array(items));
                    }
                    items.push(self.value_at(depth + 1)?);
                    self.skip_ws();
                    match self.peek() {
                        Some(b',') => self.pos += 1,
                        Some(b']') => {}
                        _ => return Err(self.err("expected ',' or ']'")),
                    }
                }
            }
            Some(b'{') => {
                self.pos += 1;
                let mut fields = Vec::new();
                loop {
                    self.skip_ws();
                    match self.peek() {
                        Some(b'}') => {
                            self.pos += 1;
                            return Ok(Value::Object(fields));
                        }
                        Some(b'"') => {}
                        _ => return Err(self.err("expected a string key")),
                    }
                    let key = self.string()?;
                    self.skip_ws();
                    if self.peek() != Some(b':') {
                        return Err(self.err("expected ':'"));
                    }
                    self.pos += 1;
                    let val = self.value_at(depth + 1)?;
                    fields.push((key, val));
                    self.skip_ws();
                    match self.peek() {
                        Some(b',') => self.pos += 1,
                        Some(b'}') => {}
                        _ => return Err(self.err("expected ',' or '}'")),
                    }
                }
            }
            Some(b'"') => self.string().map(Value::Scalar),
            Some(c) if c == b'-' || c.is_ascii_alphanumeric() => {
                let start = self.pos;
                while self
                    .peek()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'+' | b'.'))
                {
                    self.pos += 1;
                }
                let word = String::from_utf8_lossy(&self.s[start..self.pos]).into_owned();
                Ok(Value::Scalar(word))
            }
            None => Err(self.err("unexpected end of file")),
            Some(_) => Err(self.err("expected a value")),
        }
    }

    /// A string at `pos` (on its opening quote).
    fn string(&mut self) -> Result<String, Error> {
        self.pos += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let Some(c) = self.peek() else {
                return Err(self.err("unterminated string"));
            };
            self.pos += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let Some(e) = self.peek() else {
                        return Err(self.err("unterminated string"));
                    };
                    self.pos += 1;
                    let ch = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => self.unicode_escape()?,
                        _ => return Err(self.err("invalid escape")),
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                c => out.push(c),
            }
        }
        Ok(String::from_utf8_lossy(&out).into_owned())
    }

    /// The `XXXX` of `\uXXXX` (and a following low surrogate).
    fn unicode_escape(&mut self) -> Result<char, Error> {
        let hi = self.hex4()?;
        let code = if (0xD800..0xDC00).contains(&hi) {
            if self.s.get(self.pos..self.pos + 2) != Some(b"\\u") {
                return Err(self.err("unpaired surrogate"));
            }
            self.pos += 2;
            let lo = self.hex4()?;
            if !(0xDC00..0xE000).contains(&lo) {
                return Err(self.err("unpaired surrogate"));
            }
            0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
        } else {
            hi
        };
        char::from_u32(code).ok_or_else(|| self.err("invalid \\u escape"))
    }

    fn hex4(&mut self) -> Result<u32, Error> {
        let digits =
            self.s.get(self.pos..self.pos + 4).ok_or_else(|| self.err("short \\u escape"))?;
        let text = std::str::from_utf8(digits).map_err(|_| self.err("invalid \\u escape"))?;
        let v = u32::from_str_radix(text, 16).map_err(|_| self.err("invalid \\u escape"))?;
        self.pos += 4;
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape_of(text: &str) -> String {
        match parse(text) {
            Err(Error::Shape(m)) => m,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn entries() {
        let db = parse(
            r#"[
              {"directory": "/d", "file": "a.cpp", "command": "c++ -c a.cpp -o a.o", "output": "a.o"},
              {"directory": ".", "file": "b.cpp", "arguments": ["c++", "-c", "b.cpp"],
               "command": "ignored"},
            ]"#,
        )
        .unwrap();
        assert_eq!(db.len(), 2);
        assert_eq!(db[0].command, ["c++", "-c", "a.cpp", "-o", "a.o"]);
        assert_eq!(db[0].output.as_deref(), Some("a.o"));
        assert_eq!(db[1].command, ["c++", "-c", "b.cpp"]);
        assert_eq!(db[1].directory, ".");
        assert!(parse("[]").unwrap().is_empty());
        // Scalars are taken by their text, as clang's YAML reader does.
        let db = parse(r#"[{"directory": 3, "file": "a", "arguments": ["x", 4]}]"#).unwrap();
        assert_eq!(db[0].directory, "3");
        assert_eq!(db[0].command, ["x", "4"]);
    }

    #[test]
    fn shape_errors() {
        assert_eq!(shape_of(""), "Expected array.");
        assert_eq!(shape_of("{}"), "Expected array.");
        assert_eq!(shape_of("[3]"), "Expected object.");
        assert_eq!(shape_of(r#"[{"directory": "."}]"#), "Missing key: \"file\".");
        assert_eq!(shape_of(r#"[{"file": "a"}]"#), "Missing key: \"command\" or \"arguments\".");
        assert_eq!(shape_of(r#"[{"file": "a", "command": "x"}]"#), "Missing key: \"directory\".");
        assert_eq!(
            shape_of(r#"[{"directory": ".", "file": "a", "arguments": "y"}]"#),
            "Expected sequence as value."
        );
        assert_eq!(
            shape_of(r#"[{"directory": ".", "file": "a", "command": ["x"]}]"#),
            "Expected string as value."
        );
        assert_eq!(
            shape_of(r#"[{"directory": ".", "file": "a", "command": "x", "zork": 1}]"#),
            "Unknown key: \"\"zork\"\""
        );
    }

    #[test]
    fn syntax_errors() {
        assert_eq!(
            parse("[\n {\"a\" 1}]"),
            Err(Error::Syntax { line: 2, col: 7, what: "expected ':'" })
        );
        assert!(matches!(parse("[\"x"), Err(Error::Syntax { .. })));
        assert!(matches!(parse("[1] 2"), Err(Error::Syntax { .. })));
        let deep = "[".repeat(200);
        assert!(matches!(parse(&deep), Err(Error::Syntax { what: "nesting too deep", .. })));
    }

    #[test]
    fn strings() {
        let db = parse(r#"[{"directory": "é😀\n", "file": "a\"b", "command": "x"}]"#).unwrap();
        assert_eq!(db[0].directory, "\u{e9}\u{1f600}\n");
        assert_eq!(db[0].file, "a\"b");
    }

    #[test]
    fn command_splitting() {
        assert_eq!(
            split_command(r#"c++  -c 's t.cpp' -o "a \"b\".o" -DX=\"q\" a\ b"#),
            ["c++", "-c", "s t.cpp", "-o", "a \"b\".o", "-DX=\"q\"", "a b"]
        );
        assert!(split_command("  ").is_empty());
        assert_eq!(split_command("a'b'\"c\"d"), ["abcd"]);
    }
}
