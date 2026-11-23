//! A pull reader of JSON events over any byte stream: no document is
//! built, so a dump of gigabytes is read in constant memory (spec/harness.md
//! "Expression extents").

use std::io::{self, Read};

/// One event of the document, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// `{`.
    Obj,
    /// `}`.
    EndObj,
    /// `[`.
    Arr,
    /// `]`.
    EndArr,
    /// An object's key (the value follows).
    Key(String),
    Str(String),
    /// A number: its value when it is an integer that fits.
    Num(Option<i64>),
    Bool(bool),
    Null,
    /// The end of the document.
    End,
}

/// Where the reader is in a container.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ctx {
    /// In an object, before a key (or `}`).
    Key,
    /// In an object, before a value.
    Value,
    /// In an array.
    Arr,
}

/// The pull reader.
pub struct Reader<R> {
    src: R,
    buf: Box<[u8]>,
    pos: usize,
    len: usize,
    stack: Vec<Ctx>,
    /// The container just opened has had no element yet.
    fresh: bool,
    done: bool,
    /// Bytes consumed (for error messages).
    offset: u64,
}

/// The nesting limit (each AST level is two JSON levels).
const MAX_DEPTH: usize = 100_000;

fn bad(at: u64, what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("JSON at byte {at}: {what}"))
}

impl<R: Read> Reader<R> {
    pub fn new(src: R) -> Self {
        Reader {
            src,
            buf: vec![0; 1 << 16].into_boxed_slice(),
            pos: 0,
            len: 0,
            stack: Vec::new(),
            fresh: false,
            done: false,
            offset: 0,
        }
    }

    fn peek(&mut self) -> io::Result<Option<u8>> {
        if self.pos == self.len {
            self.len = loop {
                match self.src.read(&mut self.buf) {
                    Ok(n) => break n,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                    Err(e) => return Err(e),
                }
            };
            self.pos = 0;
            if self.len == 0 {
                return Ok(None);
            }
        }
        Ok(Some(self.buf[self.pos]))
    }

    fn bump(&mut self) {
        self.pos += 1;
        self.offset += 1;
    }

    fn next_byte(&mut self) -> io::Result<u8> {
        let b = self.peek()?.ok_or_else(|| bad(self.offset, "unexpected end"))?;
        self.bump();
        Ok(b)
    }

    fn skip_ws(&mut self) -> io::Result<Option<u8>> {
        while let Some(b) = self.peek()? {
            if !matches!(b, b' ' | b'\t' | b'\n' | b'\r') {
                return Ok(Some(b));
            }
            self.bump();
        }
        Ok(None)
    }

    fn expect(&mut self, word: &[u8]) -> io::Result<()> {
        for &w in word {
            if self.next_byte()? != w {
                return Err(bad(self.offset, "bad literal"));
            }
        }
        Ok(())
    }

    fn hex4(&mut self) -> io::Result<u32> {
        let mut v = 0;
        for _ in 0..4 {
            let b = self.next_byte()?;
            let d = (b as char).to_digit(16).ok_or_else(|| bad(self.offset, "bad \\u escape"))?;
            v = v * 16 + d;
        }
        Ok(v)
    }

    fn string(&mut self) -> io::Result<String> {
        let mut out = Vec::new();
        loop {
            let b = self.next_byte()?;
            match b {
                b'"' => break,
                b'\\' => match self.next_byte()? {
                    b'"' => out.push(b'"'),
                    b'\\' => out.push(b'\\'),
                    b'/' => out.push(b'/'),
                    b'b' => out.push(8),
                    b'f' => out.push(12),
                    b'n' => out.push(b'\n'),
                    b'r' => out.push(b'\r'),
                    b't' => out.push(b'\t'),
                    b'u' => {
                        let mut c = self.hex4()?;
                        if (0xD800..0xDC00).contains(&c) {
                            self.expect(b"\\u")?;
                            let lo = self.hex4()?;
                            c = 0x10000 + ((c - 0xD800) << 10) + (lo.wrapping_sub(0xDC00) & 0x3FF);
                        }
                        let ch = char::from_u32(c).unwrap_or('\u{FFFD}');
                        let mut tmp = [0; 4];
                        out.extend_from_slice(ch.encode_utf8(&mut tmp).as_bytes());
                    }
                    _ => return Err(bad(self.offset, "bad escape")),
                },
                _ => out.push(b),
            }
        }
        Ok(String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into()))
    }

    fn number(&mut self, first: u8) -> io::Result<Option<i64>> {
        let mut text = vec![first];
        while let Some(b) = self.peek()? {
            if !(b.is_ascii_digit() || matches!(b, b'-' | b'+' | b'.' | b'e' | b'E')) {
                break;
            }
            text.push(b);
            self.bump();
        }
        Ok(std::str::from_utf8(&text).ok().and_then(|t| t.parse().ok()))
    }

    /// A value starting with byte `b` (already consumed).
    fn value(&mut self, b: u8) -> io::Result<Event> {
        Ok(match b {
            b'{' => {
                if self.stack.len() >= MAX_DEPTH {
                    return Err(bad(self.offset, "nested too deeply"));
                }
                self.stack.push(Ctx::Key);
                self.fresh = true;
                Event::Obj
            }
            b'[' => {
                if self.stack.len() >= MAX_DEPTH {
                    return Err(bad(self.offset, "nested too deeply"));
                }
                self.stack.push(Ctx::Arr);
                self.fresh = true;
                Event::Arr
            }
            b'"' => Event::Str(self.string()?),
            b't' => {
                self.expect(b"rue")?;
                Event::Bool(true)
            }
            b'f' => {
                self.expect(b"alse")?;
                Event::Bool(false)
            }
            b'n' => {
                self.expect(b"ull")?;
                Event::Null
            }
            b'-' | b'0'..=b'9' => Event::Num(self.number(b)?),
            _ => return Err(bad(self.offset, "unexpected byte")),
        })
    }

    /// The next event.
    pub fn next(&mut self) -> io::Result<Event> {
        if self.done {
            return Ok(Event::End);
        }
        let Some(b) = self.skip_ws()? else {
            if self.stack.is_empty() {
                self.done = true;
                return Ok(Event::End);
            }
            return Err(bad(self.offset, "unexpected end"));
        };
        self.bump();
        match self.stack.last().copied() {
            None => {
                let ev = self.value(b)?;
                if !matches!(ev, Event::Obj | Event::Arr) {
                    self.done = true;
                }
                Ok(ev)
            }
            Some(Ctx::Key) => {
                let b = match (b, self.fresh) {
                    (b'}', _) => {
                        self.stack.pop();
                        self.after_value();
                        return Ok(Event::EndObj);
                    }
                    (b',', false) => {
                        self.skip_ws()?.ok_or_else(|| bad(self.offset, "unexpected end"))?;
                        self.next_byte()?
                    }
                    (b, true) => b,
                    _ => return Err(bad(self.offset, "expected `,` or `}`")),
                };
                if b != b'"' {
                    return Err(bad(self.offset, "expected a key"));
                }
                let k = self.string()?;
                if self.skip_ws()? != Some(b':') {
                    return Err(bad(self.offset, "expected `:`"));
                }
                self.bump();
                self.fresh = false;
                if let Some(c) = self.stack.last_mut() {
                    *c = Ctx::Value;
                }
                Ok(Event::Key(k))
            }
            Some(Ctx::Value) => {
                if let Some(c) = self.stack.last_mut() {
                    *c = Ctx::Key;
                }
                let ev = self.value(b)?;
                if !matches!(ev, Event::Obj | Event::Arr) {
                    self.fresh = false;
                }
                Ok(ev)
            }
            Some(Ctx::Arr) => {
                let b = match (b, self.fresh) {
                    (b']', _) => {
                        self.stack.pop();
                        self.after_value();
                        return Ok(Event::EndArr);
                    }
                    (b',', false) => {
                        self.skip_ws()?.ok_or_else(|| bad(self.offset, "unexpected end"))?;
                        self.next_byte()?
                    }
                    (b, true) => b,
                    _ => return Err(bad(self.offset, "expected `,` or `]`")),
                };
                let ev = self.value(b)?;
                if !matches!(ev, Event::Obj | Event::Arr) {
                    self.fresh = false;
                }
                Ok(ev)
            }
        }
    }

    /// A container closed: its parent has had an element now.
    fn after_value(&mut self) {
        self.fresh = false;
        if self.stack.is_empty() {
            self.done = true;
        }
    }
}
