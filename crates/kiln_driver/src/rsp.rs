//! Response files (`@file`), as clang's driver expands them
//! (`llvm::cl::ExpansionContext::expandResponseFiles` with the GNU
//! tokenizer): spec/driver.md "Response files".

use std::path::Path;

/// The nesting depth after which an `@file` is left as written (a cycle).
const MAX_DEPTH: usize = 32;

/// `argv` with each `@file` argument replaced by the arguments in `file`,
/// recursively. An `@file` that cannot be read stays as it is (it is then
/// an input that does not exist, as in clang). A nested `@file` is
/// relative to the current directory.
#[must_use]
pub fn expand(argv: &[String]) -> Vec<String> {
    if !argv.iter().any(|a| a.len() > 1 && a.starts_with('@')) {
        return argv.to_vec();
    }
    let mut out = Vec::with_capacity(argv.len());
    expand_into(argv, 0, &mut out);
    out
}

fn expand_into(argv: &[String], depth: usize, out: &mut Vec<String>) {
    for a in argv {
        let Some(path) = a.strip_prefix('@').filter(|p| !p.is_empty()) else {
            out.push(a.clone());
            continue;
        };
        if depth >= MAX_DEPTH {
            out.push(a.clone());
            continue;
        }
        match std::fs::read(Path::new(path)) {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                expand_into(&tokenize_gnu(&text), depth + 1, out);
            }
            Err(_) => out.push(a.clone()),
        }
    }
}

/// Splits `s` as `llvm::cl::TokenizeGNUCommandLine`: whitespace separates
/// arguments; `'` and `"` quote (an empty pair is an empty argument); a
/// backslash takes the next character literally, inside quotes too.
#[must_use]
pub fn tokenize_gnu(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut tok = String::new();
    let mut in_tok = false;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                if in_tok {
                    out.push(std::mem::take(&mut tok));
                    in_tok = false;
                }
            }
            '\\' => {
                in_tok = true;
                if let Some(n) = chars.next() {
                    tok.push(n);
                }
            }
            '\'' | '"' => {
                in_tok = true;
                while let Some(n) = chars.next() {
                    if n == c {
                        break;
                    }
                    if n == '\\'
                        && let Some(m) = chars.next()
                    {
                        tok.push(m);
                        continue;
                    }
                    tok.push(n);
                }
            }
            c => {
                in_tok = true;
                tok.push(c);
            }
        }
    }
    if in_tok {
        out.push(tok);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_like_gnu() {
        assert_eq!(tokenize_gnu("a b\n\tc"), ["a", "b", "c"]);
        assert_eq!(tokenize_gnu("'a b' \"c d\" e\\ f"), ["a b", "c d", "e f"]);
        assert_eq!(tokenize_gnu("x\"\" '' y"), ["x", "", "y"]);
        assert_eq!(tokenize_gnu("\"a\\\"b\" -DX=\\\"s\\\""), ["a\"b", "-DX=\"s\""]);
        assert!(tokenize_gnu("  \n").is_empty());
    }

    #[test]
    fn expands_nested_files() {
        let dir = std::env::temp_dir();
        let inner = dir.join(format!("kiln-rsp-inner-{}", std::process::id()));
        let outer = dir.join(format!("kiln-rsp-outer-{}", std::process::id()));
        std::fs::write(&inner, "b.o 'c d.o'\n").unwrap();
        std::fs::write(&outer, format!("a.o @{}\n-o x", inner.display())).unwrap();
        let v = vec!["-c".to_owned(), format!("@{}", outer.display()), "e.o".to_owned()];
        let got = expand(&v);
        let _ = std::fs::remove_file(&inner);
        let _ = std::fs::remove_file(&outer);
        assert_eq!(got, ["-c", "a.o", "b.o", "c d.o", "-o", "x", "e.o"]);
    }

    #[test]
    fn missing_file_and_plain_at_stay() {
        let v = vec!["@".to_owned(), "@/nonexistent/kiln-rsp".to_owned(), "-c".to_owned()];
        assert_eq!(expand(&v), v);
    }
}
