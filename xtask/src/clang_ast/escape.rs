//! String literals: clang's `StringLiteral::outputString` spelling (octal
//! `\ooo`, `\xHEX`, `\u`/`\U`) decoded into code units and re-spelled the
//! kiln way (`spec/ast-dump.md` §2; `kiln_ast::strings::push_unit`).

use std::fmt::Write as _;

/// The code-unit width of a literal, from its prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unit {
    /// Ordinary and `u8`: bytes.
    Byte,
    /// `u`: UTF-16 code units.
    Utf16,
    /// `U`, and `L` on the 32-bit-`wchar_t` targets the harness runs on.
    Utf32,
}

/// Re-spells clang's printed literal `lit` (prefix and quotes included).
pub fn respell(lit: &str) -> Result<String, String> {
    let bad = |why: &str| format!("clang-ast: string literal {lit}: {why}");
    let open = lit.find('"').ok_or_else(|| bad("no opening quote"))?;
    let (prefix, rest) = lit.split_at(open);
    let body = rest
        .strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .ok_or_else(|| bad("not a quoted literal"))?;
    let unit = match prefix {
        "" | "u8" => Unit::Byte,
        "u" => Unit::Utf16,
        "U" | "L" => Unit::Utf32,
        _ => return Err(bad("unknown encoding prefix")),
    };
    let units = decode(body, unit).map_err(|e| bad(&e))?;
    let mut out = String::with_capacity(lit.len() + 2);
    out.push_str(prefix);
    out.push('"');
    for u in units {
        push_unit(u, &mut out);
    }
    out.push('"');
    Ok(out)
}

/// kiln's spelling of one code unit inside `"..."`.
pub fn push_unit(u: u32, out: &mut String) {
    match u {
        0x5c => out.push_str("\\\\"),
        0x0a => out.push_str("\\n"),
        0x09 => out.push_str("\\t"),
        0x0d => out.push_str("\\r"),
        0x00 => out.push_str("\\0"),
        0x07 => out.push_str("\\a"),
        0x08 => out.push_str("\\b"),
        0x0c => out.push_str("\\f"),
        0x0b => out.push_str("\\v"),
        0x22 => out.push_str("\\\""),
        0x20..=0x7e => out.push(char::from(u as u8)),
        _ => {
            let _ = write!(out, "\\x{{{u:x}}}");
        }
    }
}

fn decode(body: &str, unit: Unit) -> Result<Vec<u32>, String> {
    let b = body.as_bytes();
    let mut units = Vec::with_capacity(b.len());
    let mut i = 0;
    let hex_run = |from: usize, max: usize| {
        let mut j = from;
        while j < b.len() && j - from < max && b[j].is_ascii_hexdigit() {
            j += 1;
        }
        j
    };
    while i < b.len() {
        match b[i] {
            b'\\' => {
                let c = *b.get(i + 1).ok_or("dangling backslash")?;
                i += 2;
                let simple = match c {
                    b'\\' => Some(0x5c),
                    b'"' => Some(0x22),
                    b'\'' => Some(0x27),
                    b'?' => Some(0x3f),
                    b'a' => Some(0x07),
                    b'b' => Some(0x08),
                    b'f' => Some(0x0c),
                    b'n' => Some(0x0a),
                    b'r' => Some(0x0d),
                    b't' => Some(0x09),
                    b'v' => Some(0x0b),
                    _ => None,
                };
                if let Some(v) = simple {
                    units.push(v);
                    continue;
                }
                match c {
                    b'0'..=b'7' => {
                        let start = i - 1;
                        let mut end = start;
                        while end < b.len() && end - start < 3 && (b'0'..=b'7').contains(&b[end]) {
                            end += 1;
                        }
                        let v = u32::from_str_radix(&body[start..end], 8)
                            .map_err(|_| "bad octal escape")?;
                        units.push(v);
                        i = end;
                    }
                    b'x' => {
                        let end = hex_run(i, 8);
                        if end == i {
                            return Err("empty \\x escape".into());
                        }
                        let v =
                            u32::from_str_radix(&body[i..end], 16).map_err(|_| "bad \\x escape")?;
                        units.push(v);
                        i = end;
                    }
                    b'u' | b'U' => {
                        let n = if c == b'u' { 4 } else { 8 };
                        let end = hex_run(i, n);
                        if end - i != n {
                            return Err("short universal character name".into());
                        }
                        let cp = u32::from_str_radix(&body[i..end], 16)
                            .map_err(|_| "bad universal character name")?;
                        i = end;
                        push_code_point(cp, unit, &mut units)?;
                    }
                    _ => return Err(format!("unknown escape `\\{}`", char::from(c))),
                }
            }
            // The `""` clang inserts after a `\x` escape followed by a hex
            // digit.
            b'"' if b.get(i + 1) == Some(&b'"') => i += 2,
            b'"' => return Err("unescaped quote".into()),
            c if c.is_ascii() => {
                units.push(u32::from(c));
                i += 1;
            }
            _ => {
                // clang escapes everything outside printable ASCII; raw
                // UTF-8 here would mean a different printer.
                return Err("raw non-ASCII character in clang's spelling".into());
            }
        }
    }
    Ok(units)
}

fn push_code_point(cp: u32, unit: Unit, units: &mut Vec<u32>) -> Result<(), String> {
    match unit {
        Unit::Byte => {
            let ch = char::from_u32(cp).ok_or("invalid code point")?;
            let mut buf = [0u8; 4];
            units.extend(ch.encode_utf8(&mut buf).bytes().map(u32::from));
        }
        Unit::Utf16 if cp > 0xffff => {
            let v = cp - 0x10000;
            units.push(0xd800 + (v >> 10));
            units.push(0xdc00 + (v & 0x3ff));
        }
        Unit::Utf16 | Unit::Utf32 => units.push(cp),
    }
    Ok(())
}
