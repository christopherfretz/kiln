//! The side channel: clang's text dump (`-Xclang -ast-dump`) of the same
//! compile, for what the JSON dump omits — the `value:` child of constexpr
//! variables; a function's `trivial`, `noexcept-unevaluated` and friend
//! words and its `Overrides:` child — and to cross-check the function
//! flags the normalizer derives from the JSON (`constexpr`, `consteval`,
//! `implicit-inline`).
//!
//! Both dumps come from clang's `ASTNodeTraverser`, so their function
//! (`FunctionDecl`, `CXXMethodDecl`, ...) and `VarDecl` nodes appear in
//! the same pre-order; the normalizer aligns them by position and checks
//! each pairing by name.

/// One line of a `value:` subtree: its label (`field`, `elements`, ...;
/// empty for an unlabeled child) and text, with its child lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VNode {
    pub label: String,
    pub text: String,
    pub children: Vec<VNode>,
}

/// The declaration classes the side channel aligns: the function kinds
/// and `VarDecl`.
pub const CLASSES: &[&str] = &[
    "FunctionDecl",
    "CXXMethodDecl",
    "CXXConstructorDecl",
    "CXXDestructorDecl",
    "CXXConversionDecl",
    "VarDecl",
    "ClassTemplateSpecializationDecl",
    "ClassTemplatePartialSpecializationDecl",
];

/// Whether `class` is one of the class template specialization classes
/// (aligned for their specialization kind, which the JSON lacks).
pub fn is_record(class: &str) -> bool {
    class.ends_with("SpecializationDecl")
}

/// A function, `VarDecl` or class template specialization line of the
/// text dump.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextDecl {
    /// One of [`CLASSES`].
    pub class: &'static str,
    /// The line without its tree prefix. For a function with a `delete
    /// message:` child, the words clang printed after the message (on the
    /// child's line) are appended.
    pub line: String,
    /// The `value:` child, for a variable clang evaluated.
    pub value: Option<VNode>,
    /// A method's `Overrides: [ ... ]` child, addresses removed
    /// (`Overrides: [ B::f 'int ()' ]`).
    pub overrides: Option<String>,
}

impl TextDecl {
    /// Whether the line names declaration `name` (` name 'type'`).
    pub fn names(&self, name: &str) -> bool {
        if is_record(self.class) {
            return self.has_flag(name);
        }
        self.line.contains(&format!(" {name} '"))
    }

    /// Whether the line carries the bare word `flag` outside its quoted
    /// type strings.
    pub fn has_flag(&self, flag: &str) -> bool {
        self.line.split('\'').step_by(2).any(|seg| seg.split(' ').any(|w| w == flag))
    }

    /// Whether the bare word `flag` follows the declaration's (first)
    /// quoted type: the specifier words of a function (`trivial`, ...),
    /// which a name before the type cannot be confused with.
    pub fn has_trailing_flag(&self, flag: &str) -> bool {
        self.line.split('\'').skip(2).step_by(2).any(|seg| seg.split(' ').any(|w| w == flag))
    }
}

/// `s` without clang's ` 0x...` addresses.
fn strip_addresses(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(" 0x") {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 3..];
        let n = tail.bytes().take_while(u8::is_ascii_hexdigit).count();
        if n == 0 {
            out.push_str(" 0x");
        }
        rest = &tail[n..];
    }
    out.push_str(rest);
    out
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

/// The function and `VarDecl` lines of `text`, in order.
pub fn parse(text: &str) -> Result<Vec<TextDecl>, String> {
    let lines: Vec<(usize, &str)> =
        text.lines().filter(|l| !l.is_empty()).map(split).collect::<Result<_, _>>()?;
    let mut decls = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let (depth, content) = lines[i];
        i += 1;
        let Some(class) = CLASSES
            .iter()
            .copied()
            .find(|c| content.strip_prefix(c).is_some_and(|r| r.starts_with(' ')))
        else {
            continue;
        };
        let mut line = content.to_owned();
        let mut value = None;
        let mut overrides = None;
        if class != "VarDecl" {
            // The `delete message:` and `Overrides:` children come first.
            while let Some(&(d, c)) = lines.get(i)
                && d == depth + 1
            {
                if let Some(msg) = c.strip_prefix("delete message: ") {
                    // clang prints the function's remaining words after
                    // the message, on the child's line.
                    if let Some(q) = msg.rfind('"') {
                        line.push_str(&msg[q + 1..]);
                    }
                } else if c.starts_with("Overrides: ") {
                    overrides = Some(strip_addresses(c));
                } else {
                    break;
                }
                i += 1;
            }
        }
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
        decls.push(TextDecl { class, line, value, overrides });
    }
    Ok(decls)
}
