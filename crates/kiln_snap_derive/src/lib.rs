//! `#[derive(Snap)]`: the encoder, decoder and schema descriptor of a type
//! for `kiln_snap` (spec/snap.md "The derive").
//!
//! Written on the compiler's own `proc_macro` API, with no dependency:
//! the grammar it needs is the item header (attributes, visibility,
//! `struct`/`enum`, generics, `where`) and the field and variant lists;
//! field types and discriminants are copied as token trees, never parsed.
//!
//! What it generates, for `struct S<T> { a: A, b: T }`:
//!
//! ```text
//! impl<T: ::kiln_snap::Snap> ::kiln_snap::Snap for S<T> {
//!     const SIZE = sum(&[<A as Snap>::SIZE, <T as Snap>::SIZE]);
//!     const SCHEMA_HASH = SchemaHash::new("struct", "S").field::<A>("a").field::<T>("b").finish();
//!     const SHAPE_HASH = SchemaHash::new("struct", "S").spelled("a", "A").spelled("b", "T").finish();
//!     fn put(&self, w) { let Self { a: f0, b: f1 } = self; f0.put(w); f1.put(w); }
//!     fn get(r) { let f0 = A::get(r)?; let f1 = T::get(r)?; Ok(Self { a: f0, b: f1 }) }
//!     fn schema(s) { if s.register::<Self>(desc) { A::schema(s); T::schema(s); } }
//! }
//! ```
//!
//! The destructuring `let Self { .. } = self` names every field and has no
//! `..`, and the decoder builds the value with a full struct literal, so a
//! field the macro did not see cannot exist. Errors are `compile_error!`
//! at the offending token, never a panic of the macro.
#![forbid(unsafe_code)]

use proc_macro::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree};

/// Derives `kiln_snap::Snap` (spec/snap.md "The derive").
///
/// Attributes:
/// - on a field: `#[snap(skip)]` (not written; restored as
///   `Default::default()`), `#[snap(with = "path::module")]` (encoded by
///   `module::put(&T, &mut Writer)` and `module::get(&mut Reader) ->
///   Result<T, SnapError>`), `#[snap(cycle)]` (the field's type reaches
///   this type again: the schema constant folds the field type's own
///   items, `SHAPE_HASH`, not its `SCHEMA_HASH`, which would depend on
///   itself; a field whose type names this type is so already);
/// - on the type: `#[snap(bound = "T: Trait, ...")]` replaces the
///   automatic `T: Snap` bound on every type parameter.
#[proc_macro_derive(Snap, attributes(snap))]
pub fn derive_snap(input: TokenStream) -> TokenStream {
    match parse(input).map(|i| expand(&i)) {
        Ok(ts) => ts,
        Err(e) => e.into_tokens(),
    }
}

// ----- errors -----------------------------------------------------------

struct Error {
    span: Span,
    msg: String,
}

fn fail<T>(span: Span, msg: impl Into<String>) -> Result<T, Error> {
    Err(Error { span, msg: msg.into() })
}

impl Error {
    /// `::core::compile_error! { "msg" }`, every token at the error's span.
    fn into_tokens(self) -> TokenStream {
        let span = self.span;
        let punct = |c, spacing| {
            let mut p = Punct::new(c, spacing);
            p.set_span(span);
            TokenTree::Punct(p)
        };
        let mut lit = Literal::string(&format!("derive(Snap): {}", self.msg));
        lit.set_span(span);
        let mut body = Group::new(Delimiter::Brace, TokenTree::Literal(lit).into());
        body.set_span(span);
        [
            punct(':', Spacing::Joint),
            punct(':', Spacing::Alone),
            TokenTree::Ident(Ident::new("core", span)),
            punct(':', Spacing::Joint),
            punct(':', Spacing::Alone),
            TokenTree::Ident(Ident::new("compile_error", span)),
            punct('!', Spacing::Alone),
            TokenTree::Group(body),
        ]
        .into_iter()
        .collect()
    }
}

// ----- input model ------------------------------------------------------

struct Input {
    name: Ident,
    params: Vec<Param>,
    /// The `where` predicates as written (without `where`).
    preds: TokenStream,
    /// `#[snap(bound = "...")]`: replaces the automatic bounds.
    bound: Option<TokenStream>,
    data: Data,
}

enum Param {
    Type { name: Ident, bounds: TokenStream },
    Const { name: Ident, ty: TokenStream },
}

enum Data {
    Struct(Fields),
    Enum(Vec<Variant>),
}

enum Fields {
    Named(Vec<Field>),
    Tuple(Vec<Field>),
    Unit,
}

impl Fields {
    fn list(&self) -> &[Field] {
        match self {
            Fields::Named(f) | Fields::Tuple(f) => f,
            Fields::Unit => &[],
        }
    }
}

struct Field {
    /// `None` for a tuple field.
    name: Option<Ident>,
    ty: TokenStream,
    how: How,
    /// `#[snap(cycle)]`.
    cycle: bool,
}

/// How a field is encoded.
enum How {
    Snap,
    Skip,
    /// `#[snap(with = "path")]`: the module path, and its text for the
    /// schema descriptor.
    With(TokenStream, String),
}

struct Variant {
    name: Ident,
    fields: Fields,
    /// The explicit discriminant's tokens, as text ("" if none).
    disc: String,
}

// ----- token cursor -----------------------------------------------------

struct Cursor {
    toks: Vec<TokenTree>,
    pos: usize,
    /// Where to report "unexpected end".
    end: Span,
}

impl Cursor {
    fn new(ts: TokenStream, end: Span) -> Self {
        Cursor { toks: ts.into_iter().collect(), pos: 0, end }
    }

    fn of(toks: Vec<TokenTree>, end: Span) -> Self {
        Cursor { toks, pos: 0, end }
    }

    fn peek(&self) -> Option<&TokenTree> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Option<TokenTree> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn span(&self) -> Span {
        self.peek().map_or(self.end, TokenTree::span)
    }

    fn at_end(&self) -> bool {
        self.pos >= self.toks.len()
    }

    fn is_punct(&self, c: char) -> bool {
        matches!(self.peek(), Some(TokenTree::Punct(p)) if p.as_char() == c)
    }

    fn is_ident(&self, s: &str) -> bool {
        matches!(self.peek(), Some(TokenTree::Ident(i)) if i.to_string() == s)
    }

    fn ident(&mut self, what: &str) -> Result<Ident, Error> {
        match self.next() {
            Some(TokenTree::Ident(i)) => Ok(i),
            Some(t) => fail(t.span(), format!("expected {what}")),
            None => fail(self.end, format!("expected {what}")),
        }
    }

    fn punct(&mut self, c: char) -> Result<(), Error> {
        if self.is_punct(c) {
            self.pos += 1;
            Ok(())
        } else {
            fail(self.span(), format!("expected `{c}`"))
        }
    }

    fn rest(&mut self) -> Vec<TokenTree> {
        let r = self.toks[self.pos..].to_vec();
        self.pos = self.toks.len();
        r
    }
}

/// Splits `toks` at top-level `sep` punctuation. With `angle`, a `sep`
/// inside `<...>` does not split (a `>` that ends `->` does not close).
fn split(toks: Vec<TokenTree>, sep: char, angle: bool) -> Vec<Vec<TokenTree>> {
    let mut out = vec![Vec::new()];
    let mut depth = 0usize;
    let mut prev_arrow = false;
    for t in toks {
        let mut arrow = false;
        if let TokenTree::Punct(p) = &t {
            let c = p.as_char();
            if angle && c == '<' {
                depth += 1;
            } else if angle && c == '>' && !prev_arrow {
                depth = depth.saturating_sub(1);
            } else if c == sep && depth == 0 {
                out.push(Vec::new());
                continue;
            }
            arrow = c == '-' && p.spacing() == Spacing::Joint;
        }
        prev_arrow = arrow;
        if let Some(last) = out.last_mut() {
            last.push(t);
        }
    }
    out.retain(|c| !c.is_empty());
    out
}

// ----- parsing ----------------------------------------------------------

/// Where a `#[snap(...)]` attribute was found.
#[derive(Clone, Copy, PartialEq)]
enum Site {
    Container,
    Field,
    Variant,
}

#[derive(Default)]
struct Attrs {
    skip: bool,
    with: Option<(TokenStream, String)>,
    bound: Option<TokenStream>,
    cycle: bool,
}

/// Consumes outer attributes, interpreting `#[snap(...)]` for `site`.
fn attrs(c: &mut Cursor, site: Site) -> Result<Attrs, Error> {
    let mut out = Attrs::default();
    while c.is_punct('#') {
        let hash = c.span();
        c.pos += 1;
        let Some(TokenTree::Group(g)) = c.next() else {
            return fail(hash, "expected an attribute");
        };
        if g.delimiter() != Delimiter::Bracket {
            return fail(g.span(), "expected an attribute");
        }
        let mut a = Cursor::new(g.stream(), g.span());
        if !a.is_ident("snap") {
            continue;
        }
        let snap = a.span();
        a.pos += 1;
        let Some(TokenTree::Group(args)) = a.next() else {
            return fail(snap, "expected `snap(...)`");
        };
        if site == Site::Variant {
            return fail(snap, "`#[snap(...)]` is not supported on an enum variant");
        }
        for arg in split(args.stream().into_iter().collect(), ',', false) {
            snap_arg(Cursor::of(arg, args.span()), site, &mut out)?;
        }
    }
    Ok(out)
}

fn snap_arg(mut a: Cursor, site: Site, out: &mut Attrs) -> Result<(), Error> {
    let key = a.ident("`skip`, `with`, `cycle` or `bound`")?;
    let k = key.to_string();
    let value = |a: &mut Cursor| -> Result<(String, Span), Error> {
        a.punct('=')?;
        let Some(TokenTree::Literal(l)) = a.next() else {
            return fail(key.span(), format!("expected `{k} = \"...\"`"));
        };
        let s = unquote(&l.to_string()).ok_or(Error {
            span: l.span(),
            msg: "expected a plain string literal without escapes".into(),
        })?;
        Ok((s, l.span()))
    };
    match (k.as_str(), site) {
        ("skip", Site::Field) => out.skip = true,
        ("cycle", Site::Field) => out.cycle = true,
        ("with", Site::Field) => {
            let (path, span) = value(&mut a)?;
            let Ok(ts) = path.parse::<TokenStream>() else {
                return fail(span, "`with` must name a module path");
            };
            out.with = Some((respan(ts, span), path));
        }
        ("bound", Site::Container) => {
            let (preds, span) = value(&mut a)?;
            let Ok(ts) = preds.parse::<TokenStream>() else {
                return fail(span, "`bound` must be `where` predicates");
            };
            out.bound = Some(respan(ts, span));
        }
        ("skip" | "with" | "cycle", _) => {
            return fail(key.span(), format!("`{k}` applies to a field"));
        }
        ("bound", _) => return fail(key.span(), "`bound` applies to the type"),
        _ => return fail(key.span(), format!("unknown snap attribute `{k}`")),
    }
    if !a.at_end() {
        return fail(a.span(), "unexpected tokens in `#[snap(...)]`");
    }
    if out.skip && out.with.is_some() {
        return fail(key.span(), "a field is either `skip` or `with`, not both");
    }
    if out.cycle && (out.skip || out.with.is_some()) {
        return fail(key.span(), "`cycle` applies to a field encoded by its type");
    }
    Ok(())
}

/// The value of a string literal token without escapes (`"..."`,
/// `r"..."`, `r#"..."#`).
fn unquote(lit: &str) -> Option<String> {
    if let Some(raw) = lit.strip_prefix('r') {
        let hashes = raw.len() - raw.trim_start_matches('#').len();
        let inner = raw.get(hashes..raw.len().checked_sub(hashes)?)?;
        return Some(inner.strip_prefix('"')?.strip_suffix('"')?.to_owned());
    }
    let inner = lit.strip_prefix('"')?.strip_suffix('"')?;
    (!inner.contains('\\')).then(|| inner.to_owned())
}

/// `ts` with every token (recursively) at `span`, so that errors in a
/// path or bound given as a string point at the string.
fn respan(ts: TokenStream, span: Span) -> TokenStream {
    ts.into_iter()
        .map(|mut t| {
            if let TokenTree::Group(g) = &t {
                let mut n = Group::new(g.delimiter(), respan(g.stream(), span));
                n.set_span(span);
                t = TokenTree::Group(n);
            } else {
                t.set_span(span);
            }
            t
        })
        .collect()
}

fn skip_vis(c: &mut Cursor) {
    if c.is_ident("pub") {
        c.pos += 1;
        if matches!(c.peek(), Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis)
        {
            c.pos += 1;
        }
    }
}

fn parse(input: TokenStream) -> Result<Input, Error> {
    let mut c = Cursor::new(input, Span::call_site());
    let container = attrs(&mut c, Site::Container)?;
    skip_vis(&mut c);
    let kw = c.ident("`struct` or `enum`")?;
    let name = c.ident("the type name")?;
    let params = if c.is_punct('<') { generics(&mut c)? } else { Vec::new() };
    let mut preds = TokenStream::new();
    if c.is_ident("where") {
        preds = where_preds(&mut c);
    }
    let data = match kw.to_string().as_str() {
        "struct" => match c.next() {
            Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => {
                Data::Struct(Fields::Named(fields(&g, true)?))
            }
            Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis => {
                let f = fields(&g, false)?;
                if c.is_ident("where") {
                    preds = where_preds(&mut c);
                }
                Data::Struct(Fields::Tuple(f))
            }
            Some(TokenTree::Punct(p)) if p.as_char() == ';' => Data::Struct(Fields::Unit),
            t => return fail(t.map_or(c.end, |t| t.span()), "expected the struct's fields"),
        },
        "enum" => match c.next() {
            Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => {
                Data::Enum(variants(&g)?)
            }
            t => return fail(t.map_or(c.end, |t| t.span()), "expected the enum's variants"),
        },
        "union" => return fail(kw.span(), "Snap cannot be derived for a union"),
        _ => return fail(kw.span(), "expected `struct` or `enum`"),
    };
    Ok(Input { name, params, preds, bound: container.bound, data })
}

/// Parses `<...>` after the type name.
fn generics(c: &mut Cursor) -> Result<Vec<Param>, Error> {
    let open = c.span();
    c.pos += 1;
    let mut toks = Vec::new();
    let mut depth = 1usize;
    let mut prev_arrow = false;
    loop {
        let Some(t) = c.next() else { return fail(open, "unclosed generic parameter list") };
        let mut arrow = false;
        if let TokenTree::Punct(p) = &t {
            match p.as_char() {
                '<' => depth += 1,
                '>' if !prev_arrow => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                '-' => arrow = p.spacing() == Spacing::Joint,
                _ => {}
            }
        }
        prev_arrow = arrow;
        toks.push(t);
    }
    let mut out = Vec::new();
    for p in split(toks, ',', true) {
        let mut pc = Cursor::of(p, open);
        attrs(&mut pc, Site::Field)?;
        if pc.is_punct('\'') {
            return fail(
                pc.span(),
                "Snap cannot be derived for a type with a lifetime parameter: \
                 a snapshot owns all of its data",
            );
        }
        if pc.is_ident("const") {
            pc.pos += 1;
            let name = pc.ident("a const parameter name")?;
            pc.punct(':')?;
            let ty = until_eq(&mut pc);
            out.push(Param::Const { name, ty });
        } else {
            let name = pc.ident("a type parameter")?;
            let bounds = if pc.is_punct(':') {
                pc.pos += 1;
                until_eq(&mut pc)
            } else {
                TokenStream::new()
            };
            out.push(Param::Type { name, bounds });
        }
    }
    Ok(out)
}

/// The tokens up to a top-level `=` (a parameter default), which is dropped.
fn until_eq(c: &mut Cursor) -> TokenStream {
    let rest = c.rest();
    let mut depth = 0usize;
    let mut out = Vec::new();
    for t in rest {
        if let TokenTree::Punct(p) = &t {
            match p.as_char() {
                '<' => depth += 1,
                '>' => depth = depth.saturating_sub(1),
                '=' if depth == 0 => break,
                _ => {}
            }
        }
        out.push(t);
    }
    out.into_iter().collect()
}

/// Consumes `where ...` up to the body (a brace group or `;`) and returns
/// the predicates.
fn where_preds(c: &mut Cursor) -> TokenStream {
    c.pos += 1;
    let mut out = Vec::new();
    while let Some(t) = c.peek() {
        match t {
            TokenTree::Group(g) if g.delimiter() == Delimiter::Brace => break,
            TokenTree::Punct(p) if p.as_char() == ';' => break,
            _ => out.push(c.next().unwrap_or_else(|| unreachable!("peeked"))),
        }
    }
    out.into_iter().collect()
}

fn fields(g: &Group, named: bool) -> Result<Vec<Field>, Error> {
    let mut out = Vec::new();
    for chunk in split(g.stream().into_iter().collect(), ',', true) {
        let mut c = Cursor::of(chunk, g.span());
        let a = attrs(&mut c, Site::Field)?;
        skip_vis(&mut c);
        let name = if named {
            let n = c.ident("a field name")?;
            c.punct(':')?;
            Some(n)
        } else {
            None
        };
        if c.at_end() {
            return fail(c.span(), "expected a field type");
        }
        let ty: TokenStream = c.rest().into_iter().collect();
        let how = match (a.skip, a.with) {
            (true, _) => How::Skip,
            (false, Some((path, text))) => How::With(path, text),
            (false, None) => How::Snap,
        };
        out.push(Field { name, ty, how, cycle: a.cycle });
    }
    Ok(out)
}

fn variants(g: &Group) -> Result<Vec<Variant>, Error> {
    let mut out = Vec::new();
    for chunk in split(g.stream().into_iter().collect(), ',', false) {
        let mut c = Cursor::of(chunk, g.span());
        attrs(&mut c, Site::Variant)?;
        let name = c.ident("a variant name")?;
        let fields = match c.peek() {
            Some(TokenTree::Group(f)) if f.delimiter() == Delimiter::Brace => {
                let f = f.clone();
                c.pos += 1;
                Fields::Named(fields(&f, true)?)
            }
            Some(TokenTree::Group(f)) if f.delimiter() == Delimiter::Parenthesis => {
                let f = f.clone();
                c.pos += 1;
                Fields::Tuple(fields(&f, false)?)
            }
            _ => Fields::Unit,
        };
        let disc = if c.is_punct('=') {
            c.pos += 1;
            c.rest().into_iter().collect::<TokenStream>().to_string()
        } else {
            String::new()
        };
        if !c.at_end() {
            return fail(c.span(), "unexpected tokens after the variant");
        }
        out.push(Variant { name, fields, disc });
    }
    Ok(out)
}

// ----- generation -------------------------------------------------------

/// A tiny `quote`: parses `template` and replaces each `$name` by the
/// tokens bound to `name`. Templates are this crate's own text; user
/// tokens only enter through the bindings, so a template that does not
/// parse is a bug of this macro.
fn q(template: &str, args: &[(&str, &TokenStream)]) -> TokenStream {
    let ts: TokenStream = template
        .parse()
        .unwrap_or_else(|e| unreachable!("derive(Snap) template {template:?} invalid: {e}"));
    subst(ts, args)
}

fn subst(ts: TokenStream, args: &[(&str, &TokenStream)]) -> TokenStream {
    let mut out = Vec::new();
    let mut it = ts.into_iter();
    while let Some(t) = it.next() {
        match t {
            TokenTree::Punct(p) if p.as_char() == '$' => {
                let Some(TokenTree::Ident(id)) = it.next() else {
                    unreachable!("derive(Snap) template: `$` not followed by a name")
                };
                let name = id.to_string();
                let Some((_, ts)) = args.iter().find(|(n, _)| *n == name) else {
                    unreachable!("derive(Snap) template: unbound `${name}`")
                };
                out.extend((*ts).clone());
            }
            TokenTree::Group(g) => {
                let mut n = Group::new(g.delimiter(), subst(g.stream(), args));
                n.set_span(g.span());
                out.push(TokenTree::Group(n));
            }
            t => out.push(t),
        }
    }
    out.into_iter().collect()
}

fn ts(t: impl Into<TokenTree>) -> TokenStream {
    TokenStream::from(t.into())
}

fn lit_str(s: &str) -> TokenStream {
    ts(Literal::string(s))
}

fn lit_usize(n: usize) -> TokenStream {
    ts(Literal::usize_suffixed(n))
}

fn cat(parts: impl IntoIterator<Item = TokenStream>) -> TokenStream {
    parts.into_iter().collect()
}

/// `parts` joined by commas.
fn comma(parts: impl IntoIterator<Item = TokenStream>) -> TokenStream {
    let mut out = TokenStream::new();
    for (i, p) in parts.into_iter().enumerate() {
        if i > 0 {
            out.extend(q(",", &[]));
        }
        out.extend(p);
    }
    out
}

fn ends_with_comma(ts: &TokenStream) -> bool {
    matches!(ts.clone().into_iter().last(), Some(TokenTree::Punct(p)) if p.as_char() == ',')
}

/// The binding of field `i` in patterns and decoders.
fn bind(i: usize) -> TokenStream {
    ts(Ident::new(&format!("__snap_f{i}"), Span::call_site()))
}

/// The field's name for the schema and error paths (`0`, `1` for tuple
/// fields; raw identifiers without `r#`).
fn field_label(f: &Field, i: usize) -> String {
    f.name.as_ref().map_or_else(
        || i.to_string(),
        |n| {
            let s = n.to_string();
            s.strip_prefix("r#").map_or_else(|| s.clone(), str::to_owned)
        },
    )
}

fn expand(inp: &Input) -> TokenStream {
    let name = ts(inp.name.clone());
    let auto_bound = inp.bound.is_none();
    let impl_params = comma(inp.params.iter().map(|p| match p {
        Param::Type { name, bounds } => {
            let n = ts(name.clone());
            match (bounds.is_empty(), auto_bound) {
                (true, true) => q("$n: ::kiln_snap::Snap", &[("n", &n)]),
                (false, true) => q("$n: $b + ::kiln_snap::Snap", &[("n", &n), ("b", bounds)]),
                (true, false) => n,
                (false, false) => q("$n: $b", &[("n", &n), ("b", bounds)]),
            }
        }
        Param::Const { name, ty } => q("const $n: $t", &[("n", &ts(name.clone())), ("t", ty)]),
    }));
    let ty_args = comma(inp.params.iter().map(|p| match p {
        Param::Type { name, .. } | Param::Const { name, .. } => ts(name.clone()),
    }));
    let (impl_generics, ty_generics) = if inp.params.is_empty() {
        (TokenStream::new(), TokenStream::new())
    } else {
        (q("<$p>", &[("p", &impl_params)]), q("<$a>", &[("a", &ty_args)]))
    };
    let mut preds = inp.preds.clone();
    if let Some(b) = &inp.bound {
        if !preds.is_empty() && !ends_with_comma(&preds) {
            preds.extend(q(",", &[]));
        }
        preds.extend(b.clone());
    }
    let where_clause =
        if preds.is_empty() { TokenStream::new() } else { q("where $p", &[("p", &preds)]) };
    let type_name = inp.name.to_string();
    let items = match &inp.data {
        Data::Struct(fields) => struct_items(&type_name, fields),
        Data::Enum(vs) => enum_items(&type_name, vs),
    };
    q(
        "#[automatically_derived]
         impl $ig ::kiln_snap::Snap for $name $tg $wc { $items }",
        &[
            ("ig", &impl_generics),
            ("name", &name),
            ("tg", &ty_generics),
            ("wc", &where_clause),
            ("items", &items),
        ],
    )
}

/// The encoded size expression of one field: `Some(n)` or `None`.
fn field_size(f: &Field) -> TokenStream {
    match &f.how {
        How::Snap => q("<$t as ::kiln_snap::Snap>::SIZE", &[("t", &f.ty)]),
        How::Skip => q("::core::option::Option::Some(0usize)", &[]),
        How::With(..) => q("::core::option::Option::None", &[]),
    }
}

/// `sum(&[sizes of the fields])`: the fixed size of a field list.
fn fields_size(fields: &Fields) -> TokenStream {
    let sizes = comma(fields.list().iter().map(field_size));
    q("::kiln_snap::__private::sum(&[$s])", &[("s", &sizes)])
}

/// The pattern binding every field of `path` (`Self`, `Self::V`): no `..`.
fn pattern(path: &TokenStream, fields: &Fields) -> TokenStream {
    let elems = |named: bool| {
        comma(fields.list().iter().enumerate().map(|(i, f)| {
            let b = if matches!(f.how, How::Skip) { q("_", &[]) } else { bind(i) };
            match (&f.name, named) {
                (Some(n), true) => q("$n: $b", &[("n", &ts(n.clone())), ("b", &b)]),
                _ => b,
            }
        }))
    };
    match fields {
        Fields::Named(_) => q("$p { $e }", &[("p", path), ("e", &elems(true))]),
        Fields::Tuple(_) => q("$p ( $e )", &[("p", path), ("e", &elems(false))]),
        Fields::Unit => path.clone(),
    }
}

/// The statements writing every field (bound by [`pattern`]) to `w`.
fn puts(fields: &Fields) -> TokenStream {
    cat(fields.list().iter().enumerate().map(|(i, f)| {
        let b = bind(i);
        match &f.how {
            How::Snap => q("<$t as ::kiln_snap::Snap>::put($b, w);", &[("t", &f.ty), ("b", &b)]),
            How::Skip => TokenStream::new(),
            How::With(path, _) => q("$p::put($b, w);", &[("p", path), ("b", &b)]),
        }
    }))
}

/// The statements reading every field from `r` into its binding; `at`
/// names the field in error paths (`Type.field`).
fn gets(fields: &Fields, at: &str) -> TokenStream {
    cat(fields.list().iter().enumerate().map(|(i, f)| {
        let b = bind(i);
        let label = lit_str(&format!("{at}.{}", field_label(f, i)));
        let call = match &f.how {
            How::Snap => q("<$t as ::kiln_snap::Snap>::get(r)", &[("t", &f.ty)]),
            How::Skip => return TokenStream::new(),
            How::With(path, _) => q("$p::get(r)", &[("p", path)]),
        };
        q(
            "let $b = ::kiln_snap::__private::field($c, $l)?;",
            &[("b", &b), ("c", &call), ("l", &label)],
        )
    }))
}

/// The value built from the bindings, skipped fields defaulted.
fn construct(path: &TokenStream, fields: &Fields) -> TokenStream {
    let value = |i: usize, f: &Field| {
        if matches!(f.how, How::Skip) {
            q("::core::default::Default::default()", &[])
        } else {
            bind(i)
        }
    };
    let elems = |named: bool| {
        comma(fields.list().iter().enumerate().map(|(i, f)| match (&f.name, named) {
            (Some(n), true) => q("$n: $v", &[("n", &ts(n.clone())), ("v", &value(i, f))]),
            _ => value(i, f),
        }))
    };
    match fields {
        Fields::Named(_) => q("$p { $e }", &[("p", path), ("e", &elems(true))]),
        Fields::Tuple(_) => q("$p ( $e )", &[("p", path), ("e", &elems(false))]),
        Fields::Unit => path.clone(),
    }
}

/// `.field::<T>("a")` / `.skip::<T>("b")` / `.with::<T>("c", "path")` for
/// the descriptor, the schema calls of the encoded field types, and the
/// same items for the constant [`SchemaHash`] (`kiln_snap`): an encoded
/// field folds its type's `SCHEMA_HASH`, except a field that closes a
/// cycle (its type names the type being derived, `self_name` or `Self`,
/// or it is marked `#[snap(cycle)]`), which folds the type's
/// `SHAPE_HASH` (its constant would depend on itself); and the items of
/// the type's own `SHAPE_HASH`, which folds every field by its spelling.
fn schema_parts(fields: &Fields, self_name: &str) -> [TokenStream; 4] {
    let mut desc = TokenStream::new();
    let mut recurse = TokenStream::new();
    let mut hash = TokenStream::new();
    let mut shape = TokenStream::new();
    for (i, f) in fields.list().iter().enumerate() {
        let label = lit_str(&field_label(f, i));
        let spelled = lit_str(&f.ty.to_string());
        match &f.how {
            How::Snap => {
                let how = if f.cycle || mentions(&f.ty, self_name) { "cycle" } else { "field" };
                let item = q(&format!(".{how}::<$t>($l)"), &[("t", &f.ty), ("l", &label)]);
                desc.extend(item.clone());
                hash.extend(item);
                recurse.extend(q("<$t as ::kiln_snap::Snap>::schema(s);", &[("t", &f.ty)]));
                shape.extend(q(".spelled($l, $s)", &[("l", &label), ("s", &spelled)]));
            }
            How::Skip => {
                desc.extend(q(".skip::<$t>($l)", &[("t", &f.ty), ("l", &label)]));
                hash.extend(q(".skip($l, $s)", &[("l", &label), ("s", &spelled)]));
                shape.extend(q(".skip($l, $s)", &[("l", &label), ("s", &spelled)]));
            }
            How::With(_, text) => {
                let path = lit_str(text);
                desc.extend(q(".with::<$t>($l, $p)", &[("t", &f.ty), ("l", &label), ("p", &path)]));
                let item = q(".with($l, $p, $s)", &[("l", &label), ("p", &path), ("s", &spelled)]);
                hash.extend(item.clone());
                shape.extend(item);
            }
        }
    }
    [desc, recurse, hash, shape]
}

/// Whether the type `ty` names `name` or `Self` anywhere in its tokens.
fn mentions(ty: &TokenStream, name: &str) -> bool {
    ty.clone().into_iter().any(|t| match t {
        TokenTree::Ident(i) => {
            let i = i.to_string();
            i == name || i == "Self"
        }
        TokenTree::Group(g) => mentions(&g.stream(), name),
        _ => false,
    })
}

fn struct_items(type_name: &str, fields: &Fields) -> TokenStream {
    let this = q("Self", &[]);
    let size = fields_size(fields);
    let pat = pattern(&this, fields);
    let put = puts(fields);
    let get = gets(fields, type_name);
    let value = construct(&this, fields);
    let [desc, recurse, hash, shape] = schema_parts(fields, type_name);
    let mut items = q(
        "const SIZE: ::core::option::Option<usize> = $size;
         const SCHEMA_HASH: u64 =
             ::kiln_snap::SchemaHash::new(\"struct\", $name) $hash .finish();
         const SHAPE_HASH: u64 =
             ::kiln_snap::SchemaHash::new(\"struct\", $name) $shape .finish();
         #[inline]
         #[allow(unused_variables)]
         fn put(&self, w: &mut ::kiln_snap::Writer) {
             let $pat = self;
             $put
         }
         #[inline]
         #[allow(unused_variables)]
         fn get(r: &mut ::kiln_snap::Reader<'_>)
             -> ::core::result::Result<Self, ::kiln_snap::SnapError> {
             $get
             ::core::result::Result::Ok($value)
         }
         fn schema(s: &mut ::kiln_snap::Schema) {
             let d = ::kiln_snap::Desc::new(\"struct\")
                 .hash(<Self as ::kiln_snap::Snap>::SCHEMA_HASH) $desc;
             if s.register::<Self>(d) { $recurse }
         }",
        &[
            ("name", &lit_str(type_name)),
            ("hash", &hash),
            ("shape", &shape),
            ("size", &size),
            ("pat", &pat),
            ("put", &put),
            ("get", &get),
            ("value", &value),
            ("desc", &desc),
            ("recurse", &recurse),
        ],
    );
    // A newtype keeps its field's `Option` encoding (a niche, such as
    // `NonZeroU32`'s 0, makes `Option<Id>` as small as `Id`).
    if let [f @ Field { how: How::Snap, .. }] = fields.list() {
        let member = f.name.as_ref().map_or_else(|| q("0", &[]), |n| ts(n.clone()));
        let rebuild = match fields {
            Fields::Named(_) => q("Self { $m: x }", &[("m", &member)]),
            _ => q("Self(x)", &[]),
        };
        items.extend(q(
            "const OPTION_SIZE: ::core::option::Option<usize> =
                 <$t as ::kiln_snap::Snap>::OPTION_SIZE;
             #[inline]
             fn put_option(v: ::core::option::Option<&Self>, w: &mut ::kiln_snap::Writer) {
                 <$t as ::kiln_snap::Snap>::put_option(v.map(|s| &s.$m), w)
             }
             #[inline]
             fn get_option(r: &mut ::kiln_snap::Reader<'_>)
                 -> ::core::result::Result<::core::option::Option<Self>, ::kiln_snap::SnapError> {
                 ::core::result::Result::Ok(
                     <$t as ::kiln_snap::Snap>::get_option(r)?.map(|x| $rebuild))
             }",
            &[("t", &f.ty), ("m", &member), ("rebuild", &rebuild)],
        ));
    }
    items
}

/// The schema items of an enum: its description, the recursion into its
/// field types, and the folds of its `SCHEMA_HASH` and `SHAPE_HASH`, each
/// variant opened by `.variant(name, discriminant)`.
fn enum_schema(type_name: &str, vs: &[Variant]) -> [TokenStream; 4] {
    let mut out: [TokenStream; 4] = Default::default();
    for v in vs {
        let [d, r, h, sh] = schema_parts(&v.fields, type_name);
        let variant = q(
            ".variant($n, $d)",
            &[("n", &lit_str(&v.name.to_string())), ("d", &lit_str(&v.disc))],
        );
        out[0].extend(variant.clone());
        out[0].extend(d);
        out[1].extend(r);
        out[2].extend(variant.clone());
        out[2].extend(h);
        out[3].extend(variant);
        out[3].extend(sh);
    }
    out
}

fn enum_items(type_name: &str, vs: &[Variant]) -> TokenStream {
    // The tag is the variant's index in declaration order, in the
    // narrowest unsigned type that holds it.
    let (tag_bytes, put_tag, get_tag): (usize, &str, &str) = match vs.len() {
        0..=256 => (1, "u8", "u8"),
        257..=65536 => (2, "u16", "u16"),
        _ => (4, "u32", "u32"),
    };
    let tag_lit = |i: usize| match tag_bytes {
        1 => ts(Literal::u8_suffixed(i as u8)),
        2 => ts(Literal::u16_suffixed(i as u16)),
        _ => ts(Literal::u32_suffixed(i as u32)),
    };
    let tag_n = lit_usize(tag_bytes);
    let put_tag = ts(Ident::new(put_tag, Span::call_site()));
    let get_tag = ts(Ident::new(get_tag, Span::call_site()));
    let sizes = comma(vs.iter().map(|v| fields_size(&v.fields)));
    let mut put_arms = TokenStream::new();
    let mut get_arms = TokenStream::new();
    let [desc, recurse, hash, shape] = enum_schema(type_name, vs);
    for (i, v) in vs.iter().enumerate() {
        let path = q("Self::$v", &[("v", &ts(v.name.clone()))]);
        let vsize = fields_size(&v.fields);
        let tag = tag_lit(i);
        put_arms.extend(q(
            "$pat => {
                 w.$pt($tag);
                 $puts
                 ::kiln_snap::__private::pad(w, Self::SIZE, $tn, $vs);
             }",
            &[
                ("pat", &pattern(&path, &v.fields)),
                ("pt", &put_tag),
                ("tag", &tag),
                ("puts", &puts(&v.fields)),
                ("tn", &tag_n),
                ("vs", &vsize),
            ],
        ));
        get_arms.extend(q(
            "$tag => {
                 $gets
                 ::kiln_snap::__private::skip_pad(r, Self::SIZE, $tn, $vs)?;
                 ::core::result::Result::Ok($value)
             }",
            &[
                ("tag", &tag),
                ("gets", &gets(&v.fields, &format!("{type_name}::{}", v.name))),
                ("tn", &tag_n),
                ("vs", &vsize),
                ("value", &construct(&path, &v.fields)),
            ],
        ));
    }
    let put_body = if vs.is_empty() {
        q("match *self {}", &[])
    } else {
        q("match self { $arms }", &[("arms", &put_arms)])
    };
    q(
        "const SIZE: ::core::option::Option<usize> =
             ::kiln_snap::__private::enum_size($tn, &[$sizes]);
         const SCHEMA_HASH: u64 =
             ::kiln_snap::SchemaHash::new(\"enum\", $name) $hash .finish();
         const SHAPE_HASH: u64 =
             ::kiln_snap::SchemaHash::new(\"enum\", $name) $shape .finish();
         #[inline]
         #[allow(unused_variables)]
         fn put(&self, w: &mut ::kiln_snap::Writer) {
             $put_body
         }
         #[inline]
         #[allow(unused_variables, unreachable_patterns)]
         fn get(r: &mut ::kiln_snap::Reader<'_>)
             -> ::core::result::Result<Self, ::kiln_snap::SnapError> {
             match r.$gt()? {
                 $arms
                 t => ::core::result::Result::Err(r.bad_tag::<Self>(u64::from(t), $tn)),
             }
         }
         fn schema(s: &mut ::kiln_snap::Schema) {
             let d = ::kiln_snap::Desc::new(\"enum\")
                 .hash(<Self as ::kiln_snap::Snap>::SCHEMA_HASH) $desc;
             if s.register::<Self>(d) { $recurse }
         }",
        &[
            ("name", &lit_str(type_name)),
            ("hash", &hash),
            ("shape", &shape),
            ("tn", &tag_n),
            ("sizes", &sizes),
            ("put_body", &put_body),
            ("gt", &get_tag),
            ("arms", &get_arms),
            ("desc", &desc),
            ("recurse", &recurse),
        ],
    )
}
