//! The side channel: clang's text dump (`-Xclang -ast-dump`) of the same
//! compile, for what the JSON dump omits — the `value:` child of constexpr
//! variables and the `destroyed` word of variables and parameters; a
//! function's `trivial`, `noexcept-unevaluated` and friend
//! words and its `Overrides:` child; a class template specialization's
//! template template and enumeration arguments — and to cross-check the function
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

/// The declaration classes the side channel aligns: the function kinds,
/// `VarDecl`, the template specialization classes and `ParmVarDecl`.
pub const CLASSES: &[&str] = &[
    "FunctionDecl",
    "CXXMethodDecl",
    "CXXConstructorDecl",
    "CXXDestructorDecl",
    "CXXConversionDecl",
    "VarDecl",
    "ClassTemplateSpecializationDecl",
    "ClassTemplatePartialSpecializationDecl",
    "VarTemplateSpecializationDecl",
    "VarTemplatePartialSpecializationDecl",
    "ParmVarDecl",
];

/// Whether `class` is one of the class template specialization classes
/// (aligned for their specialization kind, which the JSON lacks).
pub fn is_record(class: &str) -> bool {
    class.starts_with("ClassTemplate")
}

/// Whether `class` is `VarDecl` or a variable template specialization
/// class (which carry a `value:` child).
pub fn is_var(class: &str) -> bool {
    class == "VarDecl" || class.starts_with("VarTemplate")
}

/// A function, `VarDecl`, `ParmVarDecl` or class template specialization
/// line of the text dump.
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
    /// A class template (partial) specialization's `TemplateArgument`
    /// children: the JSON omits a template template argument's name and
    /// an enumeration argument's enumerator.
    pub args: Vec<TextArg>,
}

/// A `TemplateArgument` line of the text dump.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextArg {
    /// The line without its class word (`template 'q::F'`, `integral
    /// 'K::b'`, `pack '<V, W>'`).
    pub text: String,
    /// The address of the declaration on its child line (the template a
    /// template template argument names), unique within one dump.
    pub decl: Option<String>,
    /// A pack's elements.
    pub elems: Vec<TextArg>,
}

impl PartialEq<&str> for TextArg {
    fn eq(&self, other: &&str) -> bool {
        self.text == *other
    }
}

/// The parent classes under which a `TemplateTemplateParmDecl` line is a
/// template parameter (elsewhere, under a `TemplateArgument` or a type's
/// `name:` line, it is a reference to one).
pub const TEMPLATE_PARM_PARENTS: &[&str] = &[
    "ClassTemplateDecl",
    "FunctionTemplateDecl",
    "VarTemplateDecl",
    "TypeAliasTemplateDecl",
    "ConceptDecl",
    "ClassTemplatePartialSpecializationDecl",
    "VarTemplatePartialSpecializationDecl",
    "TemplateTemplateParmDecl",
];

/// A template template parameter of the text dump.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextTemplateParm {
    /// The line without its tree prefix.
    pub line: String,
    /// The address of the template its default argument names (`template
    /// <class> class TT = V`), if it has one that names a template.
    pub default: Option<String>,
}

impl TextTemplateParm {
    /// Whether the line names parameter `name` (its last word; an unnamed
    /// parameter matches the empty name).
    pub fn names(&self, name: &str) -> bool {
        name.is_empty() || self.line.ends_with(&format!(" {name}"))
    }
}

/// The address word (` 0x...`) of a declaration line.
fn address(content: &str) -> Option<String> {
    content.split(' ').nth(1).filter(|w| w.starts_with("0x")).map(str::to_owned)
}

/// The `TemplateArgument` children of the line at `depth` whose
/// descendants start `rest` (a pack's elements nested).
fn text_args(rest: &[(usize, &str)], depth: usize) -> Vec<TextArg> {
    let mut args = Vec::new();
    let kids = rest.iter().take_while(|(d, _)| *d > depth).enumerate();
    for (k, &(d, c)) in kids {
        if d != depth + 1 {
            continue;
        }
        if let Some(a) = c.strip_prefix("TemplateArgument ") {
            let child = rest.get(k + 1).filter(|(cd, _)| *cd == depth + 2);
            args.push(TextArg {
                text: a.to_owned(),
                decl: child.and_then(|(_, cc)| address(cc)),
                elems: if a.starts_with("pack ") {
                    text_args(&rest[k + 1..], d)
                } else {
                    Vec::new()
                },
            });
        }
    }
    args
}

/// The template template parameters of `text` (see
/// [`TEMPLATE_PARM_PARENTS`]), in order.
pub fn template_template_parms(text: &str) -> Result<Vec<TextTemplateParm>, String> {
    let lines: Vec<(usize, &str)> =
        text.lines().filter(|l| !l.is_empty()).map(split).collect::<Result<_, _>>()?;
    let mut parents: Vec<&str> = Vec::new();
    let mut parms = Vec::new();
    for (i, &(depth, content)) in lines.iter().enumerate() {
        parents.truncate(depth);
        let is_parm = content.starts_with("TemplateTemplateParmDecl ")
            && depth > 0
            && parents.get(depth - 1).is_some_and(|p| {
                TEMPLATE_PARM_PARENTS
                    .iter()
                    .any(|c| p.strip_prefix(c).is_some_and(|r| r.starts_with(' ')))
            });
        parents.push(content);
        if !is_parm {
            continue;
        }
        let default = text_args(&lines[i + 1..], depth)
            .into_iter()
            .find(|a| a.text.contains("template '"))
            .and_then(|a| a.decl);
        parms.push(TextTemplateParm { line: strip_addresses(content), default });
    }
    Ok(parms)
}

impl TextDecl {
    /// Whether the line names declaration `name` (` name 'type'`).
    pub fn names(&self, name: &str) -> bool {
        if is_record(self.class) {
            return self.has_flag(name);
        }
        name.is_empty() || self.line.contains(&format!(" {name} '"))
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
        if !is_var(class) && class != "ParmVarDecl" {
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
        if is_var(class)
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
        // Its direct `TemplateArgument` children; the scan does not
        // advance `i`, so nested declarations are still collected.
        let args = if is_record(class) { text_args(&lines[i..], depth) } else { Vec::new() };
        decls.push(TextDecl { class, line, value, overrides, args });
    }
    Ok(decls)
}

/// The named and functional cast classes, whose text-dump line carries
/// the type as written (`static_cast<CI>`, `functional cast to CI`),
/// which the JSON lacks (it has only the expression's type).
pub const CAST_CLASSES: &[&str] = &[
    "CXXStaticCastExpr",
    "CXXDynamicCastExpr",
    "CXXConstCastExpr",
    "CXXReinterpretCastExpr",
    "CXXFunctionalCastExpr",
];

/// The text the cast's written type follows on its line.
fn cast_head(class: &str) -> &'static str {
    match class {
        "CXXStaticCastExpr" => " static_cast<",
        "CXXDynamicCastExpr" => " dynamic_cast<",
        "CXXConstCastExpr" => " const_cast<",
        "CXXReinterpretCastExpr" => " reinterpret_cast<",
        _ => " functional cast to ",
    }
}

/// The written type of a named or functional cast line (without its
/// class): the text between [`cast_head`] (the first one outside the
/// quoted types) and the cast kind (` <Kind>`, a named cast's `>` before
/// it).
fn cast_written(class: &str, content: &str) -> Option<String> {
    let head = cast_head(class);
    let mut from = 0;
    let start = loop {
        let i = from + content[from..].find(head)?;
        if content[..i].matches('\'').count().is_multiple_of(2) {
            break i + head.len();
        }
        from = i + 1;
    };
    let end = if head.ends_with('<') { "> <" } else { " <" };
    let stop = start + content[start..].rfind(end)?;
    Some(cplusplus_spelling(&content[start..stop]))
}

/// `ty` with C++'s spelling of `bool` and of an empty parameter list:
/// clang prints a cast's written type with the default (C) printing
/// policy, so `bool` comes out `_Bool` and `void ()` `void (void)`.
fn cplusplus_spelling(ty: &str) -> String {
    respell_bool(ty).replace("(void)", "()")
}

fn respell_bool(ty: &str) -> String {
    let mut out = String::with_capacity(ty.len());
    let mut rest = ty;
    while let Some(i) = rest.find("_Bool") {
        let before = rest[..i].chars().next_back();
        let after = rest[i + 5..].chars().next();
        let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
        out.push_str(&rest[..i]);
        out.push_str(if word(before) || word(after) { "_Bool" } else { "bool" });
        rest = &rest[i + 5..];
    }
    out.push_str(rest);
    out
}

/// The named and functional cast lines of `text`, in pre-order: (class,
/// type as written).
pub fn casts(text: &str) -> Result<Vec<(&'static str, String)>, String> {
    let mut out = Vec::new();
    for l in text.lines().filter(|l| !l.is_empty()) {
        let (_, content) = split(l)?;
        let Some(class) = CAST_CLASSES
            .iter()
            .copied()
            .find(|c| content.strip_prefix(c).is_some_and(|r| r.starts_with(' ')))
        else {
            continue;
        };
        let content = strip_addresses(content);
        let w = cast_written(class, &content)
            .ok_or_else(|| format!("clang-ast: no written type in cast line `{content}`"))?;
        out.push((class, w));
    }
    Ok(out)
}

/// Written type `w` (C policy) without the tag keyword clang's C policy
/// prints before a class or enumeration that was not written elaborated
/// (an instantiation's substituted `T` prints `struct V`): a keyword is
/// kept only where the expression's JSON type `qual` has it too.
pub fn cplusplus_tags(w: &str, qual: &str) -> String {
    let mut out = String::with_capacity(w.len());
    let mut rest = w;
    'scan: while !rest.is_empty() {
        let at_word = out.chars().next_back().is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        if at_word {
            for kw in ["struct ", "class ", "union ", "enum "] {
                if let Some(tail) = rest.strip_prefix(kw) {
                    let n = tail
                        .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':'))
                        .unwrap_or(tail.len());
                    if !qual.contains(&format!("{kw}{}", &tail[..n])) {
                        rest = tail;
                        continue 'scan;
                    }
                }
            }
        }
        let c = rest.chars().next().unwrap_or(' ');
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}
