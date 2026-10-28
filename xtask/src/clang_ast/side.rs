//! The side channel: clang's text dump (`-Xclang -ast-dump`) of the same
//! compile, for what the JSON dump omits — the `value:` child of constexpr
//! variables — and to cross-check the function flags the normalizer
//! derives from the JSON (`constexpr`, `consteval`, `implicit-inline`).
//!
//! Both dumps come from clang's `ASTNodeTraverser`, so their
//! `FunctionDecl`/`VarDecl` nodes appear in the same pre-order; the
//! normalizer aligns them by position and checks each pairing by name.

/// One line of a `value:` subtree: its label (`field`, `elements`, ...;
/// empty for an unlabeled child) and text, with its child lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VNode {
    pub label: String,
    pub text: String,
    pub children: Vec<VNode>,
}

/// A `FunctionDecl` or `VarDecl` line of the text dump.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextDecl {
    /// `FunctionDecl` or `VarDecl`.
    pub class: &'static str,
    /// The line without its tree prefix.
    pub line: String,
    /// The `value:` child, for a variable clang evaluated.
    pub value: Option<VNode>,
}

impl TextDecl {
    /// Whether the line names declaration `name` (` name 'type'`).
    pub fn names(&self, name: &str) -> bool {
        self.line.contains(&format!(" {name} '"))
    }

    /// Whether the line carries the bare word `flag` outside its quoted
    /// type strings.
    pub fn has_flag(&self, flag: &str) -> bool {
        self.line.split('\'').step_by(2).any(|seg| seg.split(' ').any(|w| w == flag))
    }
}

/// Splits a dump line into its depth (0 for the root) and content.
fn split(line: &str) -> Result<(usize, &str), String> {
    let b = line.as_bytes();
    let mut i = 0;
    while i + 2 <= b.len() && (&b[i..i + 2] == b"| " || &b[i..i + 2] == b"  ") {
        i += 2;
    }
    if b[i..].starts_with(b"|-") || b[i..].starts_with(b"`-") {
        Ok((i / 2 + 1, &line[i + 2..]))
    } else if i == 0 {
        Ok((0, line))
    } else {
        Err(format!("clang-ast: unexpected text-dump line `{line}`"))
    }
}

const LABELS: &[&str] =
    &["value", "field", "fields", "element", "elements", "filler", "base", "bases"];

fn label(content: &str) -> (&str, &str) {
    if let Some((l, rest)) = content.split_once(": ")
        && LABELS.contains(&l)
    {
        return (l, rest);
    }
    ("", content)
}

/// Builds the subtree rooted at `lines[0]`; `lines` holds it and its
/// descendants only.
fn build(lines: &[(usize, &str)]) -> VNode {
    let (depth, content) = lines[0];
    let (l, text) = label(content);
    let mut node = VNode { label: l.to_owned(), text: text.to_owned(), children: Vec::new() };
    let mut i = 1;
    while i < lines.len() {
        let mut j = i + 1;
        while j < lines.len() && lines[j].0 > depth + 1 {
            j += 1;
        }
        node.children.push(build(&lines[i..j]));
        i = j;
    }
    node
}

/// The `FunctionDecl` and `VarDecl` lines of `text`, in order.
pub fn parse(text: &str) -> Result<Vec<TextDecl>, String> {
    let lines: Vec<(usize, &str)> =
        text.lines().filter(|l| !l.is_empty()).map(split).collect::<Result<_, _>>()?;
    let mut decls = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let (depth, content) = lines[i];
        i += 1;
        let class = if content.starts_with("FunctionDecl ") {
            "FunctionDecl"
        } else if content.starts_with("VarDecl ") {
            "VarDecl"
        } else {
            continue;
        };
        let mut value = None;
        if class == "VarDecl"
            && let Some(&(d, c)) = lines.get(i)
            && d == depth + 1
            && c.starts_with("value: ")
        {
            let mut j = i + 1;
            while j < lines.len() && lines[j].0 > depth + 1 {
                j += 1;
            }
            value = Some(build(&lines[i..j]));
            // The value's own lines (an `LValue` base may print
            // declarations) are not part of the declaration order.
            i = j;
        }
        decls.push(TextDecl { class, line: content.to_owned(), value });
    }
    Ok(decls)
}
