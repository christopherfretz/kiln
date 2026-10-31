//! The printer: clang JSON nodes to kiln dump lines (`spec/ast-dump.md`
//! §2–§3), with the §4 deviations applied.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use super::side::{self, TextDecl};
use super::table::{self, Disposition};
use super::values::{self, Field, Literals, Record, Types};
use super::{DumpOptions, escape};
use crate::util::json::Json;

type Res<T> = Result<T, String>;

pub(super) fn kind(n: &Json) -> &str {
    n.get("kind").and_then(Json::as_str).unwrap_or("")
}

fn flag(n: &Json, key: &str) -> bool {
    matches!(n.get(key), Some(Json::Bool(true)))
}

fn text<'a>(n: &'a Json, key: &str) -> Option<&'a str> {
    n.get(key).and_then(Json::as_str)
}

/// The operator of an overloaded operator call: the callee's name
/// (`operator+`, through its decay cast) without `operator`.
fn operator_call_spelling(n: &Json) -> Option<&str> {
    let mut callee = n.get("inner")?.as_arr()?.first()?;
    while text(callee, "kind") == Some("ImplicitCastExpr") {
        callee = callee.get("inner")?.as_arr()?.first()?;
    }
    let decl = callee.get("referencedDecl")?;
    Some(text(decl, "name")?.strip_prefix("operator")?.trim_start())
}

fn id(n: &Json) -> &str {
    text(n, "id").unwrap_or("")
}

fn name(n: &Json) -> Option<&str> {
    text(n, "name").filter(|s| !s.is_empty())
}

/// clang writes an absent child slot as `{}`.
fn is_null(n: &Json) -> bool {
    n.as_obj().is_some_and(<[_]>::is_empty)
}

/// A node's children: `inner`, or `array_filler` (an `InitListExpr` with a
/// filler, whose first entry is the filler).
fn kids(n: &Json) -> impl Iterator<Item = &Json> {
    n.as_obj()
        .into_iter()
        .flatten()
        .filter(|(k, _)| k == "inner" || k == "array_filler")
        .flat_map(|(_, v)| v.as_arr().unwrap_or(&[]))
}

/// `'type'` or `'type':'desugared'` (only when the strings differ: clang
/// 19 dumps a written `S` as `'S':'S'` through its `ElaboratedType`, which
/// kiln does not create).
fn type_pair(t: Option<&Json>, what: &str) -> Res<String> {
    let q = t
        .and_then(|t| text(t, "qualType"))
        .ok_or_else(|| format!("clang-ast: {what} has no type"))?;
    Ok(match t.and_then(|t| text(t, "desugaredQualType")) {
        Some(d) if d != q => format!("'{q}':'{d}'"),
        _ => format!("'{q}'"),
    })
}

/// The type node at the end of `t`'s sugar chain (clang's
/// `getSplitDesugaredType` over a dumped type node): each sugar node's
/// last non-argument child is the type it stands for.
fn desugar_type_node(t: &Json) -> &Json {
    const SUGAR: &[&str] = &[
        "ElaboratedType",
        "TypedefType",
        "UsingType",
        "ParenType",
        "TemplateSpecializationType",
        "SubstTemplateTypeParmType",
        "AttributedType",
        "MacroQualifiedType",
        "DecltypeType",
    ];
    let mut t = t;
    while SUGAR.contains(&kind(t)) {
        match kids(t).filter(|c| kind(c).ends_with("Type")).last() {
            Some(c) => t = c,
            None => break,
        }
    }
    t
}

fn qual_type(n: &Json) -> &str {
    n.get("type").and_then(|t| text(t, "qualType")).unwrap_or("")
}

/// The function declaration kinds.
const FN_KINDS: &[&str] = &[
    "FunctionDecl",
    "CXXMethodDecl",
    "CXXConstructorDecl",
    "CXXDestructorDecl",
    "CXXConversionDecl",
];

/// The template declaration kinds (spec/ast-dump.md §5).
const TEMPLATE_DECL_KINDS: &[&str] = &[
    "ClassTemplateDecl",
    "FunctionTemplateDecl",
    "VarTemplateDecl",
    "TypeAliasTemplateDecl",
    "ConceptDecl",
];

/// The text-dump words of a function that clang prints but the JSON dump
/// does not carry, and which kiln prints (`spec/ast-dump.md` §3).
const SIDE_FN_WORDS: &[&str] = &["trivial", "noexcept-unevaluated"];

/// Text-dump words of a function kiln has no field for.
const UNSUPPORTED_FN_WORDS: &[&str] = &["noexcept-uninstantiated", "ineligible", "not_selected"];

/// clang's text-dump words for an explicit class specialization's kind.
const EXPLICIT_SPEC_WORDS: &[&str] = &[
    "explicit_specialization",
    "explicit_instantiation_definition",
    "explicit_instantiation_declaration",
];

const INSTANTIATIONS: &str =
    "template specializations and instantiations are not in the dump yet (spec/ast-dump.md §5)";

/// What the whole tree is scanned for before printing.
#[derive(Default)]
struct Index<'a> {
    /// File names in the order clang mentions them.
    files: Vec<&'a str>,
    /// The (expansion) file of each declaration with a location.
    decl_file: HashMap<&'a str, usize>,
    types: Types,
    /// Bit-field `FieldDecl` ids.
    bitfields: HashSet<&'a str>,
    /// Functions by id (for `previousDecl`).
    funcs: HashMap<&'a str, &'a Json>,
    /// Functions whose lexical context is a class (members, and friends
    /// declared in a class).
    in_class: HashSet<&'a str>,
    /// Declarations a `FriendDecl` holds.
    befriended: HashSet<&'a str>,
    /// Functions declared in a class template specialization (their
    /// `implicit-inline` follows the pattern, which the JSON omits).
    in_spec: HashSet<&'a str>,
    /// The declaration contexts (filter mode's [`Ctx`] kinds) by id, and
    /// each declaration's nearest enclosing one (its lexical context), for
    /// the qualified name of a declaration whose semantic context differs.
    contexts: HashMap<&'a str, &'a Json>,
    lexical: HashMap<&'a str, &'a Json>,
    /// Function and `VarDecl` ids by pre-order position (the side
    /// channel's alignment).
    order: HashMap<&'a str, usize>,
    /// (class, name) of each position in `order`.
    order_names: Vec<(&'a str, &'a str)>,
    /// Declarations inside a template (whose constexpr values clang's
    /// text dump may not show).
    templated: HashSet<&'a str>,
    /// The template declarations enclosing the walk's current node.
    template_depth: usize,
    /// Some class has a non-trivial destructor, so a variable or
    /// parameter may print clang's `destroyed` (read from the side
    /// channel only then).
    nontrivial_dtor: bool,
    /// Each class template specialization's `ClassTemplateDecl` (by the
    /// specialization's id; an explicit one is listed under its template
    /// by reference).
    spec_template: HashMap<&'a str, &'a Json>,
    /// The `ClassTemplateDecl`s by name, with their lexical contexts (a
    /// partial specialization's JSON does not name its template).
    templates_by_name: HashMap<&'a str, Vec<&'a Json>>,
}

/// Whether `n` is a `CXXRecordDecl` whose destructor is non-trivial.
fn has_nontrivial_dtor(n: &Json, k: &str) -> bool {
    k == "CXXRecordDecl"
        && n.get("definitionData")
            .and_then(|d| d.get("dtor"))
            .is_some_and(|d| flag(d, "nonTrivial"))
}

impl<'a> Index<'a> {
    fn build(root: &'a Json) -> Self {
        let mut ix = Index::default();
        let mut cur = None;
        ix.walk(root, &mut cur, "", root);
        ix
    }

    /// Follows clang's "file only when it changes" location encoding in
    /// document order.
    fn scan(&mut self, v: &'a Json, cur: &mut Option<usize>) {
        match v {
            Json::Obj(o) => {
                if v.get("offset").is_some()
                    && let Some(f) = text(v, "file")
                {
                    let i = self.files.iter().position(|x| *x == f).unwrap_or_else(|| {
                        self.files.push(f);
                        self.files.len() - 1
                    });
                    *cur = Some(i);
                }
                for (_, x) in o {
                    self.scan(x, cur);
                }
            }
            Json::Arr(a) => {
                for x in a {
                    self.scan(x, cur);
                }
            }
            _ => {}
        }
    }

    /// Records a class or variable template's specializations and name,
    /// and the side-channel order of a specialization-like decl.
    fn note_template(&mut self, n: &'a Json, k: &'a str) {
        if k == "ClassTemplateDecl" || k == "VarTemplateDecl" {
            for c in kids(n).filter(|c| {
                kind(c) == "ClassTemplateSpecializationDecl"
                    || kind(c) == "VarTemplateSpecializationDecl"
            }) {
                self.spec_template.insert(id(c), n);
            }
            if let Some(nm) = name(n) {
                self.templates_by_name.entry(nm).or_default().push(n);
            }
        }
        // A specialization listed by reference has no line of its own (a
        // class one has no tag, a variable one no children).
        let by_ref = if side::is_record(k) {
            text(n, "tagUsed").is_none()
        } else {
            k == "VarTemplateSpecializationDecl" && n.get("inner").is_none()
        };
        if side::CLASSES.contains(&k) && !by_ref {
            self.order.insert(id(n), self.order_names.len());
            self.order_names.push((k, name(n).unwrap_or("")));
        }
    }

    fn walk(&mut self, n: &'a Json, cur: &mut Option<usize>, parent: &str, dc: &'a Json) {
        let Some(obj) = n.as_obj() else { return };
        let k = kind(n);
        let mut inner_dc = dc;
        if k.ends_with("Decl") {
            self.lexical.insert(id(n), dc);
            if ctx_of(n).is_some() {
                self.contexts.insert(id(n), n);
                inner_dc = n;
            }
        }
        self.nontrivial_dtor |= has_nontrivial_dtor(n, k);
        self.note_template(n, k);
        if parent == "FriendDecl" {
            self.befriended.insert(id(n));
        }
        if FN_KINDS.contains(&k) {
            self.funcs.insert(id(n), n);
            if matches!(
                parent,
                "CXXRecordDecl"
                    | "FriendDecl"
                    | "ClassTemplateSpecializationDecl"
                    | "ClassTemplatePartialSpecializationDecl"
            ) {
                self.in_class.insert(id(n));
            }
            if parent == "ClassTemplateSpecializationDecl" {
                self.in_spec.insert(id(n));
            }
        }
        if self.template_depth > 0 {
            self.templated.insert(id(n));
        }
        // A function template is transparent for its pattern's parent (a
        // member template is in its class; a friend template befriended).
        let (kid_parent, is_template) = match k {
            "FunctionTemplateDecl" => (parent, true),
            "ClassTemplatePartialSpecializationDecl" | "VarTemplatePartialSpecializationDecl" => {
                (k, true)
            }
            _ => (k, TEMPLATE_DECL_KINDS.contains(&k)),
        };
        if is_template {
            self.template_depth += 1;
        }
        let saved_depth = self.template_depth;
        let is_spec =
            k == "ClassTemplateSpecializationDecl" || k == "VarTemplateSpecializationDecl";
        if is_spec {
            self.template_depth = 0;
        }
        for (key, v) in obj {
            if key == "inner" || key == "array_filler" {
                for c in v.as_arr().unwrap_or(&[]) {
                    self.walk(c, cur, kid_parent, inner_dc);
                }
                continue;
            }
            self.scan(v, cur);
            if key == "loc"
                && k.ends_with("Decl")
                && !is_null(v)
                && let Some(f) = *cur
            {
                self.decl_file.insert(id(n), f);
            }
        }
        if is_spec {
            self.template_depth = saved_depth;
        }
        if is_template {
            self.template_depth -= 1;
        }
        match k {
            "FieldDecl" if flag(n, "isBitfield") => {
                self.bitfields.insert(id(n));
            }
            "CXXRecordDecl" if flag(n, "completeDefinition") && !flag(n, "isImplicit") => {
                if let Some(nm) = name(n) {
                    let fields = kids(n)
                        .filter(|c| kind(c) == "FieldDecl")
                        .map(|c| Field {
                            name: name(c).map(str::to_owned),
                            ty: qual_type(c).into(),
                        })
                        .collect();
                    let union = text(n, "tagUsed") == Some("union");
                    self.types.add_record(nm, Record { union, fields });
                }
            }
            "TypedefDecl" | "TypeAliasDecl" => {
                if let Some(nm) = name(n) {
                    self.types.add_alias(nm, qual_type(n));
                }
            }
            "EnumDecl" => {
                if let Some(nm) = name(n) {
                    self.types.enums.insert(nm.to_owned());
                }
            }
            _ => {}
        }
    }
}

/// A printed node.
struct Tree {
    line: String,
    kids: Vec<Tree>,
}

fn render(t: &Tree, out: &mut String) {
    out.push_str(&t.line);
    out.push('\n');
    let mut indent = String::new();
    render_kids(&t.kids, &mut indent, out);
}

fn render_kids(kids: &[Tree], indent: &mut String, out: &mut String) {
    for (i, k) in kids.iter().enumerate() {
        let last = i + 1 == kids.len();
        out.push_str(indent);
        out.push_str(if last { "`-" } else { "|-" });
        out.push_str(&k.line);
        out.push('\n');
        let len = indent.len();
        indent.push_str(if last { "  " } else { "| " });
        render_kids(&k.kids, indent, out);
        indent.truncate(len);
    }
}

/// A declaration context, for qualified names in filter mode.
enum Ctx {
    /// Contributes `name::`.
    Named(String),
    /// A function: contributes `f(params)::`, but its direct members print
    /// unqualified (`NamedDecl::printQualifiedName`).
    Function(String),
    /// Transparent (`extern "C"`, an unscoped enumeration).
    Transparent,
    /// An unnamed class (clang's spelling of it is not reproduced).
    Unnamed,
}

struct Printer<'a, 'f> {
    ix: Index<'a>,
    opts: &'f DumpOptions,
    main_file: &'f str,
    text: &'f mut (dyn FnMut() -> Result<String, String> + Send),
    side: Option<Vec<TextDecl>>,
    /// Function words derived from the JSON, checked against the side
    /// channel when it is loaded: (order position, [(word, present)]).
    fn_flags: Vec<(usize, [(&'static str, bool); 3])>,
}

/// Prints `root` (a `TranslationUnitDecl`) per `opts`.
pub fn dump(
    root: &Json,
    main_file: &str,
    opts: &DumpOptions,
    text: &mut (dyn FnMut() -> Result<String, String> + Send),
) -> Res<String> {
    if kind(root) != "TranslationUnitDecl" {
        return Err("clang-ast: the JSON dump is not a TranslationUnitDecl".into());
    }
    let mut p =
        Printer { ix: Index::build(root), opts, main_file, text, side: None, fn_flags: Vec::new() };
    let mut out = String::new();
    match &opts.filter {
        None => {
            let mut tu = Tree { line: "TranslationUnitDecl".into(), kids: Vec::new() };
            for k in kids(root) {
                if p.top_level(k)?
                    && let Some(t) = p.node(k).map_err(|e| context(k, &e))?
                {
                    tu.kids.push(t);
                }
            }
            render(&tu, &mut out);
        }
        Some(f) => {
            let mut ctx = Vec::new();
            for k in kids(root) {
                if p.top_level(k)? {
                    p.find(k, f, &mut ctx, &mut out).map_err(|e| context(k, &e))?;
                }
            }
        }
    }
    p.check_fn_flags()?;
    Ok(out)
}

fn context(n: &Json, e: &str) -> String {
    match name(n) {
        Some(nm) => format!("{e}\n  (in top-level {} `{nm}`)", kind(n)),
        None => format!("{e}\n  (in a top-level {})", kind(n)),
    }
}

impl<'a> Printer<'a, '_> {
    /// Implicit declarations are dropped unless `implicit`; an invented
    /// template parameter (`auto` in a parameter) is always printed.
    fn implicit_dropped(&self, n: &Json) -> bool {
        let k = kind(n);
        k.ends_with("Decl")
            && k != "TemplateTypeParmDecl"
            && flag(n, "isImplicit")
            && !self.opts.implicit
    }

    /// Whether a top-level declaration is printed: the main file only,
    /// unless `all-files`.
    fn top_level(&self, n: &Json) -> Res<bool> {
        if self.implicit_dropped(n) {
            return Ok(false);
        }
        if self.opts.all_files {
            return Ok(true);
        }
        match self.ix.decl_file.get(id(n)) {
            Some(&f) => Ok(self.ix.files[f] == self.main_file),
            None if flag(n, "isImplicit") => Ok(true),
            None => Err(format!(
                "clang-ast: top-level {} {} has no source location",
                kind(n),
                name(n).unwrap_or("")
            )),
        }
    }

    /// Filter mode: dumps each matching named declaration under `n`.
    fn find(&mut self, n: &'a Json, f: &str, ctx: &mut Vec<Ctx>, out: &mut String) -> Res<()> {
        if is_null(n) || self.implicit_dropped(n) {
            return Ok(());
        }
        let k = kind(n);
        if k.ends_with("Type") {
            return Ok(());
        }
        if k.ends_with("Decl")
            && let Some(nm) = name(n)
        {
            // A `UsingDecl`'s JSON name is as written (`N::x`); its
            // declaration name is `x`.
            let nm = if k == "UsingDecl" { nm.rsplit("::").next().unwrap_or(nm) } else { nm };
            let q = match text(n, "parentDeclContextId") {
                Some(p) => qualified(&self.semantic_ctx(p)?, nm)?,
                None => qualified(ctx, nm)?,
            };
            if q.contains(f) {
                if let Some(t) = self.node(n)? {
                    out.push_str("Dumping ");
                    out.push_str(&q);
                    out.push_str(":\n");
                    render(&t, out);
                    out.push('\n');
                }
                return Ok(());
            }
        }
        let pushed = ctx_of(n);
        let has = pushed.is_some();
        if let Some(c) = pushed {
            ctx.push(c);
        }
        let r = kids(n).try_for_each(|c| self.find(c, f, ctx, out));
        if has {
            ctx.pop();
        }
        r
    }

    /// The context chain of the declaration context `id` (a declaration's
    /// semantic parent), outermost first.
    fn semantic_ctx(&self, id: &str) -> Res<Vec<Ctx>> {
        let mut chain = Vec::new();
        let mut cur = self.ix.contexts.get(id).copied();
        // Bounded by the nesting depth; the cap guards a cycle.
        for _ in 0..10_000 {
            let Some(c) = cur else { break };
            if kind(c) == "TranslationUnitDecl" {
                break;
            }
            chain.extend(ctx_of(c));
            let up = text(c, "parentDeclContextId")
                .and_then(|p| self.ix.contexts.get(p))
                .or_else(|| self.ix.lexical.get(self::id(c)));
            cur = up.copied();
        }
        if cur.is_none_or(|c| kind(c) != "TranslationUnitDecl") {
            return Err(format!("clang-ast: filter mode cannot resolve the context `{id}`"));
        }
        chain.reverse();
        Ok(chain)
    }

    fn side(&mut self) -> Res<&[TextDecl]> {
        if self.side.is_none() {
            let decls = side::parse(&(self.text)()?)?;
            if decls.len() != self.ix.order_names.len() {
                return Err(format!(
                    "clang-ast: clang's text dump has {} function/variable/specialization lines but its \
                     JSON dump has {}; cannot align them",
                    decls.len(),
                    self.ix.order_names.len()
                ));
            }
            for (d, (class, nm)) in decls.iter().zip(&self.ix.order_names) {
                if d.class != *class || !d.names(nm) {
                    return Err(format!(
                        "clang-ast: text dump line `{}` does not match JSON {class} `{nm}`",
                        d.line
                    ));
                }
            }
            self.side = Some(decls);
        }
        Ok(self.side.as_deref().unwrap_or(&[]))
    }

    fn check_fn_flags(&mut self) -> Res<()> {
        let Some(side) = &self.side else { return Ok(()) };
        for (pos, words) in &self.fn_flags {
            let d = &side[*pos];
            for &(w, want) in words {
                if d.has_flag(w) != want {
                    return Err(format!(
                        "clang-ast: the derived function word `{w}` ({}) disagrees with \
                         clang's text dump `{}`",
                        if want { "present" } else { "absent" },
                        d.line
                    ));
                }
            }
        }
        Ok(())
    }

    fn kid_trees(&mut self, n: &'a Json) -> Res<Vec<Tree>> {
        let mut v = Vec::new();
        for c in kids(n) {
            if let Some(t) = self.node(c)? {
                v.push(t);
            }
        }
        Ok(v)
    }

    /// Prints one node and its subtree; `None` when the node is dropped.
    #[allow(clippy::too_many_lines, reason = "one arm per kept node kind (spec/ast-dump.md §3)")]
    fn node(&mut self, n: &'a Json) -> Res<Option<Tree>> {
        if is_null(n) {
            return Err("clang-ast: unexpected absent child (`{}`) in clang's JSON".into());
        }
        let k = kind(n);
        if self.implicit_dropped(n) {
            return Ok(None);
        }
        match table::lookup(k)? {
            Disposition::Drop => return Ok(None),
            Disposition::Unsupported(why) => {
                let nm = name(n).map(|s| format!(" `{s}`")).unwrap_or_default();
                return Err(format!("clang-ast: unsupported {k}{nm}: {why}"));
            }
            Disposition::Keep => {}
        }
        let obj = n.as_obj().unwrap_or(&[]);
        table::check_keys(k, obj.iter().map(|(key, _)| key.as_str()))?;
        let line;
        let mut kids_out = Vec::new();
        match k {
            "TypedefDecl" | "TypeAliasDecl" => {
                line = format!(
                    "{} {} {}",
                    decl_head(n, k),
                    name(n).unwrap_or(""),
                    type_pair(n.get("type"), k)?
                );
                for c in kids(n) {
                    // The type tree of the aliased type is not printed.
                    if !kind(c).ends_with("Type")
                        && let Some(t) = self.node(c)?
                    {
                        kids_out.push(t);
                    }
                }
            }
            "CXXRecordDecl" => {
                let mut l = decl_head(n, k);
                if self.ix.befriended.contains(id(n)) {
                    // A class first declared by a friend declaration (one
                    // declared before is not the friend's child).
                    if n.get("previousDecl").is_some() {
                        return Err("clang-ast: a befriended class with a previous \
                                    declaration (its friend kind is not in the JSON dump)"
                            .into());
                    }
                    push_word(&mut l, "friend_undeclared");
                }
                push_word(&mut l, text(n, "tagUsed").ok_or("clang-ast: record without a tag")?);
                push_opt(&mut l, name(n));
                if flag(n, "completeDefinition") {
                    push_word(&mut l, "definition");
                }
                line = l;
                if let Some(bases) = n.get("bases") {
                    for b in bases.as_arr().unwrap_or(&[]) {
                        kids_out.push(Tree { line: base_line(b)?, kids: Vec::new() });
                    }
                }
                kids_out.extend(self.kid_trees(n)?);
            }
            "AccessSpecDecl" => {
                let mut l = decl_head(n, k);
                push_word(&mut l, text(n, "access").ok_or("clang-ast: access without a level")?);
                line = l;
            }
            "FriendDecl" => {
                let mut l = decl_head(n, k);
                if let Some(t) = n.get("type") {
                    push_word(&mut l, &type_pair(Some(t), k)?);
                }
                line = l;
                kids_out = self.kid_trees(n)?;
            }
            "FieldDecl" => {
                let mut l = decl_head(n, k);
                push_opt(&mut l, name(n));
                push_word(&mut l, &type_pair(n.get("type"), k)?);
                if flag(n, "mutable") {
                    push_word(&mut l, "mutable");
                }
                line = l;
                kids_out = self.kid_trees(n)?;
            }
            "EnumDecl" => {
                let mut l = decl_head(n, k);
                push_opt(&mut l, text(n, "scopedEnumTag"));
                push_opt(&mut l, name(n));
                if let Some(t) = n.get("fixedUnderlyingType") {
                    push_word(&mut l, &type_pair(Some(t), "fixed underlying type")?);
                }
                line = l;
                let mut next: i128 = 0;
                for c in kids(n) {
                    if kind(c) != "EnumConstantDecl" {
                        if let Some(t) = self.node(c)? {
                            kids_out.push(t);
                        }
                        continue;
                    }
                    table::check_keys(
                        "EnumConstantDecl",
                        c.as_obj().unwrap_or(&[]).iter().map(|(key, _)| key.as_str()),
                    )?;
                    let mut line = format!(
                        "{} {} {}",
                        decl_head(c, "EnumConstantDecl"),
                        name(c).unwrap_or(""),
                        type_pair(c.get("type"), "EnumConstantDecl")?
                    );
                    // An enumeration in a template is dependent: its
                    // enumerators have no value until instantiated.
                    if !self.ix.templated.contains(id(c)) {
                        let value = match kids(c).next() {
                            Some(init) => enumerator_value(init)?,
                            None => next,
                        };
                        next = value + 1;
                        let _ = write!(line, " value={value}");
                    }
                    kids_out.push(Tree { line, kids: self.kid_trees(c)? });
                }
            }
            "LinkageSpecDecl" => {
                let mut l = decl_head(n, k);
                push_word(
                    &mut l,
                    text(n, "language").ok_or("clang-ast: linkage without language")?,
                );
                line = l;
                kids_out = self.kid_trees(n)?;
            }
            "NamespaceDecl" => {
                let mut l = decl_head(n, k);
                push_opt(&mut l, name(n));
                if flag(n, "isInline") {
                    push_word(&mut l, "inline");
                }
                if let Some(o) = n.get("originalNamespace") {
                    l.push_str(" original ");
                    l.push_str(&bare_ref(o)?);
                }
                line = l;
                kids_out = self.kid_trees(n)?;
            }
            "NamespaceAliasDecl" | "UsingDirectiveDecl" | "UsingShadowDecl" | "UsingEnumDecl" => {
                let mut l = decl_head(n, k);
                if k == "NamespaceAliasDecl" {
                    push_opt(&mut l, name(n));
                }
                let key = match k {
                    "NamespaceAliasDecl" => "aliasedNamespace",
                    "UsingDirectiveDecl" => "nominatedNamespace",
                    _ => "target",
                };
                let t = n.get(key).ok_or_else(|| format!("clang-ast: {k} without `{key}`"))?;
                l.push(' ');
                l.push_str(&bare_ref(t)?);
                if k == "UsingShadowDecl" && t.get("type").is_some() {
                    push_word(&mut l, &type_pair(t.get("type"), "shadowed declaration")?);
                }
                line = l;
                for c in kids(n) {
                    // A shadow of a type carries the type's tree (not printed).
                    if !kind(c).ends_with("Type")
                        && let Some(t) = self.node(c)?
                    {
                        kids_out.push(t);
                    }
                }
            }
            "UsingDecl" => {
                // clang's `name` is the qualified name as written (`N::x`).
                let mut l = decl_head(n, k);
                push_opt(&mut l, name(n));
                line = l;
                kids_out = self.kid_trees(n)?;
            }
            "FunctionDecl" | "CXXMethodDecl" | "CXXConstructorDecl" | "CXXDestructorDecl"
            | "CXXConversionDecl" => {
                let (l, pre) = self.function(n, k)?;
                line = l;
                kids_out = pre;
                kids_out.extend(self.kid_trees(n)?);
            }
            "CXXCtorInitializer" => {
                let mut l = String::from(k);
                if let Some(m) = n.get("anyInit") {
                    let mk = kind(m);
                    let _ = write!(
                        l,
                        " {} '{}' {}",
                        mk.strip_suffix("Decl").unwrap_or(mk),
                        text(m, "name").unwrap_or(""),
                        type_pair(m.get("type"), "initialized member")?
                    );
                } else if let Some(t) = n.get("baseInit").or_else(|| n.get("delegatingInit")) {
                    push_word(&mut l, &type_pair(Some(t), "initialized class")?);
                } else {
                    return Err("clang-ast: CXXCtorInitializer without a target".into());
                }
                line = l;
                kids_out = self.kid_trees(n)?;
            }
            "OverrideAttr" => {
                line = k.to_owned();
            }
            "FinalAttr" => {
                line = "FinalAttr final".to_owned();
            }
            "ParmVarDecl" => {
                let mut l = decl_head(n, k);
                push_opt(&mut l, name(n));
                push_word(&mut l, &type_pair(n.get("type"), k)?);
                match text(n, "init") {
                    None => {}
                    Some("c") => push_word(&mut l, "cinit"),
                    Some(other) => {
                        return Err(format!("clang-ast: default argument init style `{other}`"));
                    }
                }
                if self.destroyed(n)? {
                    push_word(&mut l, "destroyed");
                }
                if flag(n, "isParameterPack") {
                    push_word(&mut l, "pack");
                }
                line = l;
                kids_out = self.kid_trees(n)?;
            }
            "VarDecl" => {
                line = self.var(n)?;
                kids_out = self.kid_trees(n)?;
            }
            "ClassTemplateDecl"
            | "FunctionTemplateDecl"
            | "VarTemplateDecl"
            | "TypeAliasTemplateDecl"
            | "ConceptDecl" => {
                if k == "FunctionTemplateDecl"
                    && kids(n).filter(|c| FN_KINDS.contains(&kind(c))).count() > 1
                {
                    return Err(format!(
                        "clang-ast: unsupported FunctionTemplateDecl `{}` with \
                         specializations: {INSTANTIATIONS}",
                        name(n).unwrap_or("")
                    ));
                }
                let mut l = decl_head(n, k);
                push_opt(&mut l, name(n));
                line = l;
                if k == "ClassTemplateDecl" {
                    let params = spec_param_types(n);
                    for c in kids(n) {
                        if kind(c) == "ClassTemplateSpecializationDecl"
                            && text(c, "tagUsed").is_none()
                        {
                            // An explicit specialization or instantiation:
                            // printed at its lexical position.
                            kids_out.push(Tree {
                                line: format!(
                                    "ClassTemplateSpecialization '{}'",
                                    name(c).unwrap_or("")
                                ),
                                kids: Vec::new(),
                            });
                        } else if kind(c) == "ClassTemplateSpecializationDecl" {
                            kids_out.extend(self.specialization(c, params.as_deref(), false)?);
                        } else if let Some(t) = self.node(c)? {
                            kids_out.push(t);
                        }
                    }
                } else if k == "VarTemplateDecl" {
                    let params = spec_param_types(n);
                    for c in kids(n) {
                        if kind(c) == "VarTemplateSpecializationDecl" && c.get("inner").is_none() {
                            // An explicit specialization or instantiation:
                            // printed at its lexical position.
                            kids_out.push(Tree {
                                line: format!(
                                    "VarTemplateSpecialization '{}' {}",
                                    name(c).unwrap_or(""),
                                    type_pair(c.get("type"), k)?
                                ),
                                kids: Vec::new(),
                            });
                        } else if kind(c) == "VarTemplateSpecializationDecl" {
                            kids_out.extend(self.var_specialization(c, params.as_deref())?);
                        } else if let Some(t) = self.node(c)? {
                            kids_out.push(t);
                        }
                    }
                } else {
                    kids_out = self.kid_trees(n)?;
                }
            }
            "VarTemplateSpecializationDecl" => {
                // An explicit specialization or instantiation, at its
                // lexical position (or an implicit one's second line).
                let tmpl = self.ix.spec_template.get(id(n)).copied().ok_or_else(|| {
                    format!(
                        "clang-ast: specialization `{}` is not listed under a template",
                        name(n).unwrap_or("")
                    )
                })?;
                let params = spec_param_types(tmpl);
                return self.var_specialization(n, params.as_deref());
            }
            "VarTemplatePartialSpecializationDecl" => return self.var_partial(n).map(Some),
            "ClassTemplateSpecializationDecl" => {
                // An explicit specialization or instantiation, at its
                // lexical position.
                let tmpl = self.ix.spec_template.get(id(n)).copied().ok_or_else(|| {
                    format!(
                        "clang-ast: specialization `{}` is not listed under a template",
                        name(n).unwrap_or("")
                    )
                })?;
                let params = spec_param_types(tmpl);
                return self.specialization(n, params.as_deref(), true);
            }
            "ClassTemplatePartialSpecializationDecl" => return self.partial(n).map(Some),
            "TemplateTypeParmDecl" | "NonTypeTemplateParmDecl" | "TemplateTemplateParmDecl" => {
                let mut l = decl_head(n, k);
                match k {
                    "TemplateTypeParmDecl" => push_word(
                        &mut l,
                        text(n, "tagUsed").ok_or("clang-ast: type parameter without a tag")?,
                    ),
                    "NonTypeTemplateParmDecl" => push_word(&mut l, &type_pair(n.get("type"), k)?),
                    _ => {}
                }
                let num = |key: &str| {
                    n.get(key)
                        .and_then(Json::as_u64)
                        .ok_or_else(|| format!("clang-ast: {k} without `{key}`"))
                };
                let _ = write!(l, " depth {} index {}", num("depth")?, num("index")?);
                if flag(n, "isParameterPack") {
                    push_word(&mut l, "...");
                }
                push_opt(&mut l, name(n));
                line = l;
                kids_out = self.template_kids(n, k == "TemplateTemplateParmDecl")?;
            }
            "ConceptSpecializationExpr" | "UnresolvedLookupExpr" => {
                let mut l = self.expr_head(n, k)?;
                if k == "UnresolvedLookupExpr" {
                    let adl = if flag(n, "usesADL") { "(ADL)" } else { "(no ADL)" };
                    let _ = write!(l, " {adl} = '{}'", text(n, "name").unwrap_or(""));
                    if n.get("lookups").and_then(Json::as_arr).is_none_or(<[_]>::is_empty) {
                        push_word(&mut l, "empty");
                    }
                }
                line = l;
                kids_out = self.template_kids(n, false)?;
            }
            "RequiresExpr" => {
                let mut l = self.expr_head(n, k)?;
                match n.get("satisfied") {
                    Some(Json::Bool(true)) => push_word(&mut l, "satisfied"),
                    Some(Json::Bool(false)) => push_word(&mut l, "unsatisfied"),
                    _ => {}
                }
                line = l;
                kids_out = self.kid_trees(n)?;
            }
            "SimpleRequirement"
            | "TypeRequirement"
            | "CompoundRequirement"
            | "NestedRequirement" => {
                // A requirement's own `satisfied` is not printed: kiln
                // records only the requires-expression's value (stopping at
                // the first failed requirement, [expr.prim.req.general]/5).
                let mut l = String::from(k);
                if k == "TypeRequirement" {
                    // The required type is the type node child's type; the
                    // node carries no `desugaredQualType`, so the desugar
                    // is read off its sugar chain.
                    let node = kids(n).next();
                    let ty = node
                        .map(qual_type)
                        .filter(|ty| !ty.is_empty())
                        .ok_or_else(|| format!("clang-ast: {k} has no type"))?;
                    let desugared = node.map_or(ty, |t| qual_type(desugar_type_node(t)));
                    if desugared.is_empty() || desugared == ty {
                        let _ = write!(l, " '{ty}'");
                    } else {
                        let _ = write!(l, " '{ty}':'{desugared}'");
                    }
                }
                if flag(n, "noexcept") {
                    push_word(&mut l, "noexcept");
                }
                if flag(n, "isDependent") {
                    push_word(&mut l, "dependent");
                }
                line = l;
                for c in kids(n) {
                    if !kind(c).ends_with("Type")
                        && let Some(t) = self.node(c)?
                    {
                        kids_out.push(t);
                    }
                }
            }
            "CXXFoldExpr" => {
                line = self.expr_head(n, k)?;
                let mut it = kids(n);
                // The operator lookup results clang keeps as the callee.
                match it.next() {
                    Some(c) if is_null(c) => {}
                    _ => {
                        return Err("clang-ast: unsupported CXXFoldExpr with a callee (operator \
                                    candidates found by lookup)"
                            .into());
                    }
                }
                kids_out.push(Tree { line: "<<<NULL>>>".into(), kids: Vec::new() });
                for c in it {
                    if is_null(c) {
                        kids_out.push(Tree { line: "<<<NULL>>>".into(), kids: Vec::new() });
                    } else {
                        kids_out.extend(self.node(c)?);
                    }
                }
            }
            "CXXDependentScopeMemberExpr" => {
                let mut l = self.expr_head(n, k)?;
                let _ = write!(
                    l,
                    " {}{}",
                    if flag(n, "isArrow") { "->" } else { "." },
                    text(n, "member").unwrap_or("")
                );
                line = l;
                // The explicit template arguments are not printed (as in
                // clang's text dump).
                kids_out = self.kid_trees(n)?;
            }
            "CXXUnresolvedConstructExpr" => {
                let mut l = self.expr_head(n, k)?;
                if flag(n, "list") {
                    push_word(&mut l, "list");
                }
                line = l;
                kids_out = self.kid_trees(n)?;
            }
            "SizeOfPackExpr" => {
                let mut l = self.expr_head(n, k)?;
                push_opt(&mut l, name(n));
                line = l;
                kids_out = self.kid_trees(n)?;
            }
            "StaticAssertDecl" => {
                line = decl_head(n, k);
                let mut it = kids(n);
                let cond = it.next().ok_or("clang-ast: static_assert without a condition")?;
                kids_out.extend(self.node(cond)?);
                if let Some(msg) = it.next()
                    && !is_null(msg)
                {
                    kids_out.extend(self.node(msg)?);
                }
                if it.next().is_some() {
                    return Err("clang-ast: static_assert with more than two children".into());
                }
            }
            "IntegerLiteral" => {
                let v = text(n, "value").ok_or("clang-ast: IntegerLiteral without a value")?;
                line = format!("{} {v}", self.expr_head(n, k)?);
            }
            "FloatingLiteral" => {
                let v = text(n, "value").ok_or("clang-ast: FloatingLiteral without a value")?;
                line = format!("{} {}", self.expr_head(n, k)?, float_literal(qual_type(n), v)?);
            }
            "CharacterLiteral" => {
                let v = n
                    .get("value")
                    .and_then(Json::as_f64)
                    .ok_or("clang-ast: CharacterLiteral without a value")?;
                line = format!("{} {}", self.expr_head(n, k)?, v as i64);
            }
            "StringLiteral" => {
                let v = text(n, "value").ok_or("clang-ast: StringLiteral without a value")?;
                let lit = escape::respell(v)?;
                line = if qual_type(n) == "NULL TYPE" {
                    format!("StringLiteral unevaluated {lit}")
                } else {
                    format!("{} {lit}", self.expr_head(n, k)?)
                };
            }
            "CXXBoolLiteralExpr" => {
                let v = match n.get("value") {
                    Some(Json::Bool(b)) => *b,
                    _ => return Err("clang-ast: CXXBoolLiteralExpr without a value".into()),
                };
                line = format!("{} {v}", self.expr_head(n, k)?);
            }
            "DeclRefExpr" => {
                let r = n.get("referencedDecl").ok_or("clang-ast: DeclRefExpr without a decl")?;
                let rk = kind(r);
                let mut l = format!(
                    "{} {} '{}' {}",
                    self.expr_head(n, k)?,
                    rk.strip_suffix("Decl").unwrap_or(rk),
                    text(r, "name").unwrap_or(""),
                    type_pair(r.get("type"), "referenced declaration")?
                );
                if let Some(f) = n.get("foundReferencedDecl") {
                    l.push_str(" (");
                    l.push_str(&bare_ref(f)?);
                    l.push(')');
                }
                if let Some(why) = text(n, "nonOdrUseReason") {
                    l.push_str(" non_odr_use_");
                    l.push_str(why);
                }
                line = l;
                kids_out = self.kid_trees(n)?;
            }
            "MemberExpr" => {
                let arrow = if flag(n, "isArrow") { "->" } else { "." };
                let mut l = format!(
                    "{} {arrow}{}",
                    self.expr_head(n, k)?,
                    text(n, "name").ok_or("clang-ast: MemberExpr without a name")?
                );
                if let Some(why) = text(n, "nonOdrUseReason") {
                    l.push_str(" non_odr_use_");
                    l.push_str(why);
                }
                line = l;
                kids_out = self.kid_trees(n)?;
            }
            "ImplicitCastExpr"
            | "CStyleCastExpr"
            | "CXXStaticCastExpr"
            | "CXXDynamicCastExpr"
            | "CXXConstCastExpr"
            | "CXXReinterpretCastExpr"
            | "CXXFunctionalCastExpr" => {
                let ck = text(n, "castKind").ok_or("clang-ast: cast without a kind")?;
                let mut l = self.expr_head(n, k)?;
                let named = match k {
                    "CXXStaticCastExpr" => Some(" static_cast<"),
                    "CXXDynamicCastExpr" => Some(" dynamic_cast<"),
                    "CXXConstCastExpr" => Some(" const_cast<"),
                    "CXXReinterpretCastExpr" => Some(" reinterpret_cast<"),
                    "CXXFunctionalCastExpr" => Some(" functional cast to "),
                    _ => None,
                };
                if let Some(head) = named {
                    // clang prints the type as written; kiln rebuilds it
                    // from the expression's type and value category.
                    l.push_str(head);
                    l.push_str(qual_type(n));
                    match text(n, "valueCategory") {
                        Some("lvalue") => l.push_str(" &"),
                        Some("xvalue") => l.push_str(" &&"),
                        _ => {}
                    }
                    if k != "CXXFunctionalCastExpr" {
                        l.push('>');
                    }
                }
                l.push_str(" <");
                l.push_str(ck);
                if let Some(path) = n.get("path").and_then(Json::as_arr)
                    && !path.is_empty()
                {
                    let steps: Vec<String> = path
                        .iter()
                        .map(|b| {
                            let v = if flag(b, "isVirtual") { "virtual " } else { "" };
                            format!("{v}{}", text(b, "name").unwrap_or(""))
                        })
                        .collect();
                    l.push_str(" (");
                    l.push_str(&steps.join(" -> "));
                    l.push(')');
                }
                l.push('>');
                if flag(n, "isPartOfExplicitCast") {
                    l.push_str(" part_of_explicit_cast");
                }
                line = l;
                kids_out = self.kid_trees(n)?;
            }
            "UnaryOperator" => {
                let op = text(n, "opcode").ok_or("clang-ast: operator without an opcode")?;
                let fix = if flag(n, "isPostfix") { "postfix" } else { "prefix" };
                let mut l = format!("{} {fix} '{op}'", self.expr_head(n, k)?);
                if matches!(n.get("canOverflow"), Some(Json::Bool(false))) {
                    l.push_str(" cannot overflow");
                }
                line = l;
                kids_out = self.kid_trees(n)?;
            }
            "BinaryOperator" => {
                let op = text(n, "opcode").ok_or("clang-ast: operator without an opcode")?;
                line = format!("{} '{op}'", self.expr_head(n, k)?);
                kids_out = self.kid_trees(n)?;
            }
            "CompoundAssignOperator" => {
                let op = text(n, "opcode").ok_or("clang-ast: operator without an opcode")?;
                line = format!(
                    "{} '{op}' ComputeLHSTy={} ComputeResultTy={}",
                    self.expr_head(n, k)?,
                    type_pair(n.get("computeLHSType"), "ComputeLHSTy")?,
                    type_pair(n.get("computeResultType"), "ComputeResultTy")?
                );
                kids_out = self.kid_trees(n)?;
            }
            "UnaryExprOrTypeTraitExpr" => {
                let op = text(n, "name").unwrap_or("");
                if op != "sizeof" && op != "alignof" {
                    return Err(format!("clang-ast: unsupported trait `{op}` (not in M2)"));
                }
                let mut l = format!("{} {op}", self.expr_head(n, k)?);
                if let Some(t) = n.get("argType") {
                    push_word(&mut l, &type_pair(Some(t), "sizeof operand")?);
                }
                line = l;
                kids_out = self.kid_trees(n)?;
            }
            "InitListExpr" => {
                let mut l = self.expr_head(n, k)?;
                if let Some(f) = n.get("field") {
                    l.push_str(" field=");
                    l.push_str(text(f, "name").unwrap_or(""));
                }
                line = l;
                kids_out = self.kid_trees(n)?;
                if n.get("array_filler").is_some()
                    && let Some(first) = kids_out.first_mut()
                {
                    first.line.insert_str(0, "array_filler: ");
                }
            }
            "CXXConstructExpr" | "CXXTemporaryObjectExpr" => {
                let mut l = self.expr_head(n, k)?;
                push_word(&mut l, &type_pair(n.get("ctorType"), "constructor")?);
                for (key, word) in [
                    ("list", "list"),
                    ("initializer_list", "std::initializer_list"),
                    ("zeroing", "zeroing"),
                    ("isImmediateEscalating", "immediate-escalating"),
                ] {
                    if flag(n, key) {
                        push_word(&mut l, word);
                    }
                }
                line = l;
                kids_out = self.kid_trees(n)?;
            }
            "MaterializeTemporaryExpr" => {
                let mut l = self.expr_head(n, k)?;
                if let Some(d) = n.get("extendingDecl") {
                    let dk = kind(d);
                    let _ = write!(
                        l,
                        " extended by {} '{}' {}",
                        dk.strip_suffix("Decl").unwrap_or(dk),
                        text(d, "name").unwrap_or(""),
                        type_pair(d.get("type"), "extending declaration")?
                    );
                }
                line = l;
                kids_out = self.kid_trees(n)?;
            }
            "CXXBindTemporaryExpr" => {
                line = format!("{} (CXXTemporary)", self.expr_head(n, k)?);
                kids_out = self.kid_trees(n)?;
            }
            "CXXDefaultInitExpr" => {
                let mut l = self.expr_head(n, k)?;
                if flag(n, "hasRewrittenInit") {
                    l.push_str(" has rewritten init");
                }
                line = l;
                kids_out = self.kid_trees(n)?;
            }
            "CXXThisExpr" => {
                let mut l = self.expr_head(n, k)?;
                if flag(n, "implicit") {
                    l.push_str(" implicit");
                }
                l.push_str(" this");
                line = l;
            }
            "CXXTypeidExpr" => {
                let mut l = self.expr_head(n, k)?;
                if let Some(t) = n.get("typeArg") {
                    l.push_str(" type_operand=");
                    l.push_str(&type_pair(Some(t), "typeid operand")?);
                }
                line = l;
                kids_out = self.kid_trees(n)?;
            }
            "CXXParenListInitExpr" => {
                line = self.expr_head(n, k)?;
                kids_out = self.kid_trees(n)?;
                if n.get("array_filler").is_some()
                    && let Some(first) = kids_out.first_mut()
                {
                    first.line.insert_str(0, "array_filler: ");
                }
            }
            "ConstantExpr" => {
                let v = text(n, "value").ok_or_else(|| {
                    "clang-ast: ConstantExpr without a value (an lvalue or unevaluated \
                     constant) is not supported"
                        .to_string()
                })?;
                let v = values::constant(&self.ix.types, v, qual_type(n))?;
                line = format!("{} value={v}", self.expr_head(n, k)?);
                kids_out = self.kid_trees(n)?;
            }
            "CXXOperatorCallExpr" => {
                let op = operator_call_spelling(n).ok_or_else(|| {
                    "clang-ast: CXXOperatorCallExpr without an `operator` callee".to_string()
                })?;
                let adl = if flag(n, "adl") { " adl" } else { "" };
                line = format!("{} '{op}'{adl}", self.expr_head(n, k)?);
                kids_out = self.kid_trees(n)?;
            }
            "CXXNullPtrLiteralExpr"
            | "CXXNoexceptExpr"
            | "ConditionalOperator"
            | "ParenExpr"
            | "ArraySubscriptExpr"
            | "CallExpr"
            | "ImplicitValueInitExpr"
            | "ExprWithCleanups"
            | "CXXDefaultArgExpr"
            | "CXXMemberCallExpr"
            | "CXXInheritedCtorInitExpr"
            | "ArrayInitLoopExpr"
            | "ArrayInitIndexExpr"
            | "OpaqueValueExpr"
            | "CXXScalarValueInitExpr"
            | "CXXThrowExpr"
            | "StmtExpr"
            | "CompoundLiteralExpr"
            | "BinaryConditionalOperator"
            | "OffsetOfExpr"
            | "DependentScopeDeclRefExpr"
            | "SubstNonTypeTemplateParmExpr"
            | "PackExpansionExpr"
            | "CXXRewrittenBinaryOperator" => {
                line = self.expr_head(n, k)?;
                kids_out = self.kid_trees(n)?;
            }
            "CXXNewExpr" | "CXXDeleteExpr" => {
                let mut l = self.expr_head(n, k)?;
                for (key, word) in [("isGlobal", " global"), ("isArray", " array")] {
                    if flag(n, key) {
                        l.push_str(word);
                    }
                }
                let key = if k == "CXXNewExpr" { "operatorNewDecl" } else { "operatorDeleteDecl" };
                if let Some(r) = n.get(key) {
                    l.push(' ');
                    l.push_str(&bare_ref(r)?);
                    l.push(' ');
                    l.push_str(&type_pair(r.get("type"), "allocation function")?);
                }
                line = l;
                kids_out = self.kid_trees(n)?;
            }
            "IfStmt" => {
                let mut l = String::from(k);
                for (key, word) in
                    [("hasInit", "has_init"), ("hasVar", "has_var"), ("hasElse", "has_else")]
                {
                    if flag(n, key) {
                        push_word(&mut l, word);
                    }
                }
                if flag(n, "isConstexpr") {
                    push_word(&mut l, "constexpr");
                }
                if flag(n, "isConsteval") {
                    push_word(
                        &mut l,
                        if flag(n, "constevalIsNegated") { "!consteval" } else { "consteval" },
                    );
                }
                line = l;
                kids_out = self.kid_trees(n)?;
            }
            "SwitchStmt" | "WhileStmt" => {
                let mut l = String::from(k);
                for (key, word) in [("hasInit", "has_init"), ("hasVar", "has_var")] {
                    if flag(n, key) {
                        push_word(&mut l, word);
                    }
                }
                line = l;
                kids_out = self.kid_trees(n)?;
            }
            "ForStmt" => {
                let slots: Vec<&Json> = kids(n).collect();
                if slots.len() != 5 {
                    return Err(format!(
                        "clang-ast: ForStmt with {} child slots, not 5",
                        slots.len()
                    ));
                }
                let mut l = String::from(k);
                for (slot, word) in slots.iter().zip(["has_init", "has_var", "has_cond", "has_inc"])
                {
                    if !is_null(slot) {
                        push_word(&mut l, word);
                    }
                }
                line = l;
                for slot in slots {
                    if !is_null(slot)
                        && let Some(t) = self.node(slot)?
                    {
                        kids_out.push(t);
                    }
                }
            }
            "CompoundStmt" | "DeclStmt" | "NullStmt" | "ReturnStmt" | "BreakStmt"
            | "ContinueStmt" | "DefaultStmt" | "CaseStmt" | "DoStmt" => {
                line = k.to_owned();
                kids_out = self.kid_trees(n)?;
            }
            _ => return Err(format!("clang-ast: no printer for kept kind `{k}` (normalizer bug)")),
        }
        Ok(Some(Tree { line, kids: kids_out }))
    }

    /// `Class 'type' [lvalue|xvalue] [bitfield]`.
    fn expr_head(&self, n: &Json, class: &str) -> Res<String> {
        let mut s = format!("{class} {}", type_pair(n.get("type"), class)?);
        match text(n, "valueCategory") {
            Some("prvalue") => {}
            Some(c @ ("lvalue" | "xvalue")) => {
                push_word(&mut s, c);
                if self.bitfield(n) {
                    push_word(&mut s, "bitfield");
                }
            }
            other => return Err(format!("clang-ast: {class} with value category {other:?}")),
        }
        Ok(s)
    }

    /// Whether a glvalue designates a bit-field (clang's `OK_BitField`,
    /// which the JSON dump omits): derived as Sema propagates it.
    fn bitfield(&self, n: &Json) -> bool {
        if text(n, "valueCategory") == Some("prvalue") {
            return false;
        }
        let kid = |i: usize| kids(n).nth(i).is_some_and(|c| self.bitfield(c));
        match kind(n) {
            "MemberExpr" => {
                text(n, "referencedMemberDecl").is_some_and(|d| self.ix.bitfields.contains(d))
            }
            "ParenExpr" | "CompoundAssignOperator" => kid(0),
            "SubstNonTypeTemplateParmExpr" => kid(1),
            "BinaryOperator" => match text(n, "opcode") {
                Some("=") => kid(0),
                Some(",") => kid(1),
                _ => false,
            },
            "UnaryOperator" => {
                !flag(n, "isPostfix") && matches!(text(n, "opcode"), Some("++" | "--")) && kid(0)
            }
            "ConditionalOperator" => kid(1) || kid(2),
            _ => false,
        }
    }

    /// Whether a function's line needs the side channel: for the words
    /// only clang's text dump has (`friend`, `trivial`,
    /// `noexcept-unevaluated`) and the `Overrides:` child, which only
    /// members, friends and defaulted functions can carry.
    fn needs_side(&self, n: &Json, class: &str) -> bool {
        class != "FunctionDecl"
            || self.ix.befriended.contains(id(n))
            || n.get("explicitlyDefaulted").is_some()
    }

    /// A function's line, and the children clang prints before its
    /// parameters (the `delete message:` and `Overrides:` lines).
    fn function(&mut self, n: &'a Json, class: &str) -> Res<(String, Vec<Tree>)> {
        let consteval = flag(n, "immediate");
        let constexpr = flag(n, "constexpr") && !consteval;
        let deleted = flag(n, "explicitlyDeleted");
        let inline = flag(n, "inline");
        let pos = self.ix.order.get(id(n)).copied();
        let side = match pos {
            Some(pos) if self.needs_side(n, class) => Some(self.side()?[pos].clone()),
            _ => None,
        };
        // An instantiated member is inline as its pattern is (clang's
        // JSON omits the body of a member not yet used).
        let implicit_inline = !inline
            && match &side {
                Some(d) if self.ix.in_spec.contains(id(n)) => d.has_flag("implicit-inline"),
                _ => self.inlined(n),
            };
        if let Some(pos) = pos {
            self.fn_flags.push((
                pos,
                [
                    ("constexpr", constexpr),
                    ("consteval", consteval),
                    ("implicit-inline", implicit_inline),
                ],
            ));
        }
        let nm = name(n).ok_or_else(|| format!("clang-ast: {class} without a name"))?;
        let mut l = self.head(n, class);
        if constexpr {
            push_word(&mut l, "constexpr");
        }
        if consteval {
            push_word(&mut l, "consteval");
        }
        if let Some(d) = &side {
            if let Some(w) = UNSUPPORTED_FN_WORDS.iter().find(|w| d.has_trailing_flag(w)) {
                return Err(format!("clang-ast: unsupported function word `{w}` on `{}`", d.line));
            }
            for w in ["friend", "friend_undeclared"] {
                if d.has_flag(w) {
                    push_word(&mut l, w);
                }
            }
        }
        push_word(&mut l, nm);
        push_word(&mut l, &type_pair(n.get("type"), class)?);
        push_opt(&mut l, text(n, "storageClass"));
        for (key, word) in [("inline", "inline"), ("virtual", "virtual"), ("pure", "pure")] {
            if flag(n, key) {
                push_word(&mut l, word);
            }
        }
        match text(n, "explicitlyDefaulted") {
            None => {}
            Some("default") => push_word(&mut l, "default"),
            Some("deleted") => push_word(&mut l, "default_delete"),
            Some(other) => return Err(format!("clang-ast: explicitlyDefaulted `{other}`")),
        }
        if deleted {
            push_word(&mut l, "delete");
        }
        if let Some(d) = &side {
            for w in SIDE_FN_WORDS {
                if d.has_trailing_flag(w) {
                    push_word(&mut l, w);
                }
            }
        }
        if implicit_inline {
            push_word(&mut l, "implicit-inline");
        }
        let mut pre = Vec::new();
        if let Some(msg) = text(n, "deletedMessage") {
            let mut line = String::from("delete message: StringLiteral unevaluated \"");
            for b in msg.bytes() {
                escape::push_unit(u32::from(b), &mut line);
            }
            line.push('"');
            pre.push(Tree { line, kids: Vec::new() });
        }
        if let Some(o) = side.and_then(|d| d.overrides) {
            pre.push(Tree { line: o, kids: Vec::new() });
        }
        Ok((l, pre))
    }

    /// `FunctionDecl::isInlined()`: `inline`, `constexpr`/`consteval` and
    /// deleted functions, and those defined (or defaulted) in their class,
    /// are inline, and a redeclaration inherits it.
    fn inlined(&self, n: &Json) -> bool {
        let mut cur = Some(n);
        // Bounded by the redeclaration chain; the cap guards a cycle.
        for _ in 0..10_000 {
            let Some(f) = cur else { return false };
            if flag(f, "inline") || flag(f, "constexpr") || flag(f, "explicitlyDeleted") {
                return true;
            }
            if self.ix.in_class.contains(id(f))
                && (f.get("explicitlyDefaulted").is_some()
                    || kids(f).any(|c| kind(c).ends_with("Stmt")))
            {
                return true;
            }
            cur = text(f, "previousDecl").and_then(|p| self.ix.funcs.get(p).copied());
        }
        false
    }

    /// [`decl_head`] for a function or variable. clang also prints `parent`
    /// on a block-scope `extern` declaration (its semantic context is the
    /// enclosing namespace); kiln's dump does not model that, so the word is
    /// dropped there.
    fn head(&self, n: &Json, class: &str) -> String {
        let block_scope = self.ix.lexical.get(id(n)).is_some_and(|dc| FN_KINDS.contains(&kind(dc)));
        let l = decl_head(n, class);
        if block_scope { l.replacen(" parent", "", 1) } else { l }
    }

    fn var(&mut self, n: &'a Json) -> Res<String> {
        let class = kind(n);
        let mut l = self.head(n, class);
        push_word(&mut l, name(n).ok_or("clang-ast: VarDecl without a name")?);
        push_word(&mut l, &type_pair(n.get("type"), "VarDecl")?);
        match class {
            "VarTemplatePartialSpecializationDecl" => {
                push_word(&mut l, "explicit_specialization");
            }
            "VarTemplateSpecializationDecl" => {
                // clang's JSON lacks the specialization kind: the text
                // dump has it.
                let pos = self.ix.order.get(id(n)).copied().ok_or_else(|| {
                    format!(
                        "clang-ast: specialization `{}` is not in the side-channel order",
                        name(n).unwrap_or("")
                    )
                })?;
                let d = &self.side()?[pos];
                let word = ["implicit_instantiation"]
                    .iter()
                    .chain(EXPLICIT_SPEC_WORDS)
                    .find(|w| d.has_flag(w))
                    .ok_or_else(|| format!("clang-ast: no specialization kind on `{}`", d.line))?;
                push_word(&mut l, word);
            }
            _ => {}
        }
        push_opt(&mut l, text(n, "storageClass"));
        for key in ["nrvo", "inline", "constexpr"] {
            if flag(n, key) {
                push_word(&mut l, key);
            }
        }
        if let Some(init) = text(n, "init") {
            push_word(
                &mut l,
                match init {
                    "c" => "cinit",
                    "call" => "callinit",
                    "list" => "listinit",
                    "paren-list" => "parenlistinit",
                    _ => return Err(format!("clang-ast: unsupported init style `{init}`")),
                },
            );
            if flag(n, "constexpr")
                && let Some(v) = self.var_value(n)?
            {
                l.push_str(" value=");
                l.push_str(&v);
            }
        }
        if self.destroyed(n)? {
            push_word(&mut l, "destroyed");
        }
        Ok(l)
    }

    /// clang's `destroyed` (`VarDecl::needsDestruction`), which only the
    /// text dump shows.
    fn destroyed(&mut self, n: &'a Json) -> Res<bool> {
        if !self.ix.nontrivial_dtor {
            return Ok(false);
        }
        let Some(&pos) = self.ix.order.get(id(n)) else { return Ok(false) };
        Ok(self.side()?[pos].has_trailing_flag("destroyed"))
    }

    /// The `value=` of a constexpr variable, from the side channel (none
    /// for a variable in a template whose value clang did not compute).
    fn var_value(&mut self, n: &'a Json) -> Res<Option<String>> {
        let pos = *self.ix.order.get(id(n)).ok_or("clang-ast: VarDecl missing from the index")?;
        let templated = self.ix.templated.contains(id(n));
        let side = self.side()?;
        let vnode = match side[pos].value.clone() {
            Some(v) => v,
            None if templated => return Ok(None),
            None => {
                return Err(format!(
                    "clang-ast: clang's text dump shows no value for constexpr `{}`",
                    name(n).unwrap_or("")
                ));
            }
        };
        let v = values::parse(&vnode)?;
        let mut lits = Literals::default();
        for c in kids(n) {
            collect_literals(c, &mut lits);
        }
        let null_init = kids(n).next().is_some_and(is_null_pointer_constant);
        values::print_var(&self.ix.types, &v, qual_type(n), &lits, null_init).map(Some)
    }

    /// A `TemplateArgument` line (spec/ast-dump.md §5). clang's JSON dump
    /// writes a template-name argument as an empty object, which cannot be
    /// told from a pack expansion of one; it is accepted only as a template
    /// template parameter's default (`ttp_default`).
    fn template_arg(&mut self, n: &'a Json, ttp_default: bool) -> Res<Tree> {
        const KEYS: &[&str] = &["kind", "range", "inner", "type", "isExpr"];
        if let Some((key, _)) =
            n.as_obj().unwrap_or(&[]).iter().find(|(key, _)| !KEYS.contains(&key.as_str()))
        {
            return Err(match key.as_str() {
                "inherited from" | "previous" => {
                    "clang-ast: unsupported inherited default template argument (kiln prints \
                     a default on the declaring parameter only)"
                        .into()
                }
                _ => format!(
                    "clang-ast: `TemplateArgument` attribute `{key}` is not in the mapping \
                     table (only type, expression and template arguments as written are)"
                ),
            });
        }
        let mut t = Tree { line: String::from("TemplateArgument"), kids: Vec::new() };
        if let Some(ty) = n.get("type") {
            push_word(&mut t.line, "type");
            push_word(&mut t.line, &type_pair(Some(ty), "TemplateArgument")?);
            // The type's tree is not printed.
        } else if flag(n, "isExpr") {
            push_word(&mut t.line, "expr");
            for c in kids(n) {
                t.kids.extend(self.node(c)?);
            }
        } else if ttp_default && kids(n).next().is_none() {
            push_word(&mut t.line, "template");
        } else {
            return Err("clang-ast: unsupported template-name TemplateArgument (clang's JSON \
                        dump does not say whether it is a pack expansion)"
                .into());
        }
        Ok(t)
    }

    /// An implicit instantiation printed under its `ClassTemplateDecl`
    /// (spec/ast-dump.md §5): the record line plus `implicit_instantiation`
    /// when defined, the converted arguments, then the members. `params`
    /// holds each template parameter's type (`None` for a type or
    /// template parameter), or is `None` when it has a pack.
    fn specialization(
        &mut self,
        n: &'a Json,
        params: Option<&[Option<String>]>,
        explicit: bool,
    ) -> Res<Option<Tree>> {
        let nm = name(n).unwrap_or("");
        let Some(params) = params else {
            return Err(format!(
                "clang-ast: unsupported specialization of `{nm}`, a variadic template (WP-072)"
            ));
        };
        if self.implicit_dropped(n) {
            return Ok(None);
        }
        table::check_keys(
            "ClassTemplateSpecializationDecl",
            n.as_obj().unwrap_or(&[]).iter().map(|(key, _)| key.as_str()),
        )?;
        let mut line = decl_head(n, "ClassTemplateSpecializationDecl");
        push_word(&mut line, text(n, "tagUsed").unwrap_or(""));
        push_word(&mut line, nm);
        if explicit {
            // clang's JSON lacks the specialization kind: the text dump
            // has it.
            if flag(n, "completeDefinition") {
                push_word(&mut line, "definition");
            }
            let pos = self.ix.order.get(id(n)).copied().ok_or_else(|| {
                format!("clang-ast: specialization `{nm}` is not in the side-channel order")
            })?;
            let d = &self.side()?[pos];
            let word = EXPLICIT_SPEC_WORDS.iter().find(|w| d.has_flag(w)).ok_or_else(|| {
                format!("clang-ast: no explicit specialization kind on `{}`", d.line)
            })?;
            push_word(&mut line, word);
        } else if flag(n, "completeDefinition") {
            push_word(&mut line, "definition");
            push_word(&mut line, "implicit_instantiation");
        }
        let mut tree = Tree { line, kids: Vec::new() };
        let mut index = 0;
        let mut args = Vec::new();
        let mut rest = Vec::new();
        for child in kids(n) {
            if kind(child) == "TemplateArgument" {
                let param = params.get(index).ok_or_else(|| {
                    format!("clang-ast: `{nm}` has more arguments than parameters")
                })?;
                args.push(spec_arg(child, param.as_deref())?);
                index += 1;
            } else {
                rest.push(child);
            }
        }
        tree.kids.extend(args);
        if let Some(bases) = n.get("bases") {
            for base in bases.as_arr().unwrap_or(&[]) {
                tree.kids.push(Tree { line: base_line(base)?, kids: Vec::new() });
            }
        }
        for child in rest {
            if let Some(x) = self.node(child)? {
                tree.kids.push(x);
            }
        }
        Ok(Some(tree))
    }

    /// A variable template specialization (spec/ast-dump.md §5): the
    /// variable line with its specialization kind, the converted
    /// arguments, then the initializer. `params` as for
    /// [`Self::specialization`].
    fn var_specialization(
        &mut self,
        n: &'a Json,
        params: Option<&[Option<String>]>,
    ) -> Res<Option<Tree>> {
        let nm = name(n).unwrap_or("");
        let Some(params) = params else {
            return Err(format!(
                "clang-ast: unsupported specialization of `{nm}`, a variadic template (WP-072)"
            ));
        };
        if self.implicit_dropped(n) {
            return Ok(None);
        }
        table::check_keys(
            "VarTemplateSpecializationDecl",
            n.as_obj().unwrap_or(&[]).iter().map(|(key, _)| key.as_str()),
        )?;
        let mut tree = Tree { line: self.var(n)?, kids: Vec::new() };
        let mut index = 0;
        for child in kids(n) {
            if kind(child) == "TemplateArgument" {
                let param = params.get(index).ok_or_else(|| {
                    format!("clang-ast: `{nm}` has more arguments than parameters")
                })?;
                tree.kids.push(spec_arg(child, param.as_deref())?);
                index += 1;
            } else {
                tree.kids.extend(self.node(child)?);
            }
        }
        Ok(Some(tree))
    }

    /// A variable template partial specialization, at its lexical
    /// position (spec/ast-dump.md §5): the variable line plus
    /// `explicit_specialization`, then (in clang's order) the template
    /// parameters, the canonical arguments and the initializer.
    fn var_partial(&mut self, n: &'a Json) -> Res<Tree> {
        const KIND: &str = "VarTemplatePartialSpecializationDecl";
        let nm = name(n).unwrap_or("");
        table::check_keys(KIND, n.as_obj().unwrap_or(&[]).iter().map(|(key, _)| key.as_str()))?;
        let tmpl = self.primary_of(n)?;
        let params = spec_param_types(tmpl).ok_or_else(|| {
            format!("clang-ast: unsupported partial specialization of `{nm}`, a variadic template (WP-072)")
        })?;
        let mut tree = Tree { line: self.var(n)?, kids: Vec::new() };
        let mut index = 0;
        for child in kids(n) {
            if kind(child) == "TemplateArgument" {
                let param = params.get(index).ok_or_else(|| {
                    format!("clang-ast: partial `{nm}` has more arguments than parameters")
                })?;
                let t = if child.get("value").is_some() {
                    spec_arg(child, param.as_deref())?
                } else {
                    self.template_arg(child, false)?
                };
                tree.kids.push(t);
                index += 1;
            } else {
                tree.kids.extend(self.node(child)?);
            }
        }
        Ok(tree)
    }

    /// The primary template of partial specialization `n`: the one of
    /// that name in the same lexical context, else the only one of that
    /// name.
    fn primary_of(&self, n: &'a Json) -> Res<&'a Json> {
        let nm = name(n).unwrap_or("");
        let cands = self.ix.templates_by_name.get(nm).map_or(&[][..], Vec::as_slice);
        let here = self.ix.lexical.get(id(n)).map(|c| id(c));
        cands
            .iter()
            .find(|t| self.ix.lexical.get(id(t)).map(|c| id(c)) == here)
            .or(if cands.len() == 1 { cands.first() } else { None })
            .copied()
            .ok_or_else(|| format!("clang-ast: cannot find the template of partial `{nm}`"))
    }

    /// A class template partial specialization, at its lexical position
    /// (spec/ast-dump.md §5): the record line plus `explicit_specialization`,
    /// the canonical arguments, the template parameters, the bases, then
    /// the members.
    fn partial(&mut self, n: &'a Json) -> Res<Tree> {
        const KIND: &str = "ClassTemplatePartialSpecializationDecl";
        let nm = name(n).unwrap_or("");
        table::check_keys(KIND, n.as_obj().unwrap_or(&[]).iter().map(|(key, _)| key.as_str()))?;
        let tmpl = self.primary_of(n)?;
        let params = spec_param_types(tmpl).ok_or_else(|| {
            format!("clang-ast: unsupported partial specialization of `{nm}`, a variadic template (WP-072)")
        })?;
        let mut line = decl_head(n, KIND);
        push_word(&mut line, text(n, "tagUsed").unwrap_or(""));
        push_word(&mut line, nm);
        if flag(n, "completeDefinition") {
            push_word(&mut line, "definition");
        }
        push_word(&mut line, "explicit_specialization");
        let mut tree = Tree { line, kids: Vec::new() };
        let mut index = 0;
        let mut rest = Vec::new();
        for child in kids(n) {
            match kind(child) {
                "TemplateArgument" => {
                    let param = params.get(index).ok_or_else(|| {
                        format!("clang-ast: partial `{nm}` has more arguments than parameters")
                    })?;
                    let t = if child.get("value").is_some() {
                        spec_arg(child, param.as_deref())?
                    } else {
                        self.template_arg(child, false)?
                    };
                    tree.kids.push(t);
                    index += 1;
                }
                "TemplateTypeParmDecl" | "NonTypeTemplateParmDecl" | "TemplateTemplateParmDecl" => {
                    tree.kids.extend(self.node(child)?);
                }
                _ => rest.push(child),
            }
        }
        if let Some(bases) = n.get("bases") {
            for base in bases.as_arr().unwrap_or(&[]) {
                tree.kids.push(Tree { line: base_line(base)?, kids: Vec::new() });
            }
        }
        for child in rest {
            tree.kids.extend(self.node(child)?);
        }
        Ok(tree)
    }

    /// The children of a template parameter, a concept-id or an
    /// unresolved name: `TemplateArgument` children go through
    /// [`Self::template_arg`].
    fn template_kids(&mut self, n: &'a Json, ttp_default: bool) -> Res<Vec<Tree>> {
        let mut v = Vec::new();
        for c in kids(n) {
            if kind(c) == "TemplateArgument" {
                v.push(self.template_arg(c, ttp_default)?);
            } else if !kind(c).ends_with("Type")
                && let Some(t) = self.node(c)?
            {
                v.push(t);
            }
        }
        Ok(v)
    }
}

/// The type of each parameter of the class template `n` (`None` for a
/// type or template parameter); `None` when one is a pack.
fn spec_param_types(n: &Json) -> Option<Vec<Option<String>>> {
    let mut v = Vec::new();
    for c in kids(n) {
        match kind(c) {
            "TemplateTypeParmDecl" | "TemplateTemplateParmDecl" | "NonTypeTemplateParmDecl"
                if flag(c, "isParameterPack") =>
            {
                return None;
            }
            "TemplateTypeParmDecl" | "TemplateTemplateParmDecl" => v.push(None),
            "NonTypeTemplateParmDecl" => v.push(Some(qual_type(c).to_owned())),
            _ => {}
        }
    }
    Some(v)
}

/// A converted argument of an implicit instantiation: `TemplateArgument
/// type '<type>'` or `value '<type>' <integer>` (spec/ast-dump.md §5).
/// clang's JSON writes an integral value as its sign-extended 64-bit
/// pattern (`true` is -1), so the parameter's type `param` decodes it.
fn spec_arg(n: &Json, param: Option<&str>) -> Res<Tree> {
    let mut line = String::from("TemplateArgument");
    if let Some(ty) = n.get("type") {
        push_word(&mut line, "type");
        push_word(&mut line, &type_pair(Some(ty), "TemplateArgument")?);
        return Ok(Tree { line, kids: Vec::new() });
    }
    // A converted argument is never a pack expansion (packs are `pack`
    // arguments with children), so clang's empty object here is a
    // template-name argument.
    if n.as_obj().is_some_and(|o| o.len() == 1) {
        push_word(&mut line, "template");
        return Ok(Tree { line, kids: Vec::new() });
    }
    // Numbers parse as `f64`: exact within 2^53 (the bound), enough for
    // the corpus.
    let as_i64 = |j: &Json| {
        j.as_f64()
            .filter(|f| f.fract() == 0.0 && f.abs() <= 9_007_199_254_740_992.0)
            .map(|f| f as i64)
    };
    let (Some(v), Some(p)) = (n.get("value").and_then(as_i64), param) else {
        return Err("clang-ast: unsupported non-integral template argument of a \
                    specialization (WP-071)"
            .into());
    };
    let value: i128 = match p {
        "bool" => i128::from(v != 0),
        "unsigned char" | "char8_t" => i128::from(v as u8),
        "unsigned short" | "char16_t" => i128::from(v as u16),
        "unsigned int" | "char32_t" => i128::from(v as u32),
        "unsigned long" | "unsigned long long" | "size_t" | "std::size_t" => i128::from(v as u64),
        "int" | "long" | "long long" | "short" | "signed char" | "char" | "wchar_t" => {
            i128::from(v)
        }
        _ => {
            return Err(format!(
                "clang-ast: unsupported template argument of type `{p}` in a specialization"
            ));
        }
    };
    push_word(&mut line, "value");
    let _ = write!(line, " '{p}' {value}");
    Ok(Tree { line, kids: Vec::new() })
}

fn push_word(l: &mut String, w: &str) {
    l.push(' ');
    l.push_str(w);
}

fn push_opt(l: &mut String, w: Option<&str>) {
    if let Some(w) = w {
        push_word(l, w);
    }
}

/// `Kind 'name'`: a reference to a declaration without its type (clang's
/// `dumpBareDeclRef` of a non-value declaration).
fn bare_ref(r: &Json) -> Res<String> {
    let rk = kind(r);
    let rk = rk.strip_suffix("Decl").ok_or_else(|| format!("clang-ast: `{rk}` is not a decl"))?;
    Ok(format!("{rk} '{}'", text(r, "name").unwrap_or("")))
}

/// `Class [parent] [implicit] [used|referenced]`; `parent` when the
/// semantic context differs from the lexical one.
fn decl_head(n: &Json, class: &str) -> String {
    let mut l = String::from(class);
    if n.get("parentDeclContextId").is_some() {
        l.push_str(" parent");
    }
    if flag(n, "isImplicit") {
        l.push_str(" implicit");
    }
    if flag(n, "isUsed") {
        l.push_str(" used");
    } else if flag(n, "isReferenced") {
        l.push_str(" referenced");
    }
    l
}

/// A base-specifier line, `[virtual ]access 'B'[...]` (the semantic
/// access).
fn base_line(b: &Json) -> Res<String> {
    const KEYS: &[&str] = &["access", "writtenAccess", "isVirtual", "type", "isPackExpansion"];
    if let Some((k, _)) =
        b.as_obj().into_iter().flatten().find(|(k, _)| !KEYS.contains(&k.as_str()))
    {
        return Err(format!("clang-ast: base-specifier attribute `{k}` is not in the mapping"));
    }
    let mut l = String::new();
    if flag(b, "isVirtual") {
        l.push_str("virtual ");
    }
    l.push_str(text(b, "access").ok_or("clang-ast: base-specifier without an access")?);
    l.push(' ');
    l.push_str(&type_pair(b.get("type"), "base class")?);
    if flag(b, "isPackExpansion") {
        l.push_str("...");
    }
    Ok(l)
}

/// The filter-mode context a declaration opens, if it is one.
fn ctx_of(n: &Json) -> Option<Ctx> {
    match kind(n) {
        "CXXRecordDecl" => Some(name(n).map_or(Ctx::Unnamed, |s| Ctx::Named(s.into()))),
        "EnumDecl" if n.get("scopedEnumTag").is_some() => {
            Some(name(n).map_or(Ctx::Unnamed, |s| Ctx::Named(s.into())))
        }
        "TranslationUnitDecl" | "EnumDecl" | "LinkageSpecDecl" => Some(Ctx::Transparent),
        // `NamedDecl::printQualifiedName` skips inline namespaces.
        "NamespaceDecl" if flag(n, "isInline") => Some(Ctx::Transparent),
        "NamespaceDecl" => Some(Ctx::Named(name(n).unwrap_or("(anonymous namespace)").to_owned())),
        k if FN_KINDS.contains(&k) => Some(Ctx::Function(signature(n))),
        _ => None,
    }
}

/// The qualified name of `nm` declared in `ctx`
/// (`NamedDecl::printQualifiedName`).
fn qualified(ctx: &[Ctx], nm: &str) -> Res<String> {
    if matches!(ctx.last(), Some(Ctx::Function(_))) {
        return Ok(nm.to_owned());
    }
    let mut q = String::new();
    for c in ctx {
        match c {
            Ctx::Named(s) | Ctx::Function(s) => {
                q.push_str(s);
                q.push_str("::");
            }
            Ctx::Transparent => {}
            Ctx::Unnamed => {
                return Err(format!(
                    "clang-ast: filter mode cannot name `{nm}`, a member of an unnamed class \
                     or enumeration"
                ));
            }
        }
    }
    q.push_str(nm);
    Ok(q)
}

/// `f(int, char *, ...)`, as a function prints in a qualified name.
fn signature(n: &Json) -> String {
    let params: Vec<&str> = kids(n).filter(|c| kind(c) == "ParmVarDecl").map(qual_type).collect();
    let mut s = format!("{}({}", name(n).unwrap_or(""), params.join(", "));
    if flag(n, "variadic") {
        s.push_str(if params.is_empty() { "..." } else { ", ..." });
    }
    s.push(')');
    s
}

/// An enumerator's value from its initializer's `ConstantExpr`.
fn enumerator_value(init: &Json) -> Res<i128> {
    let mut e = init;
    while matches!(kind(e), "ImplicitCastExpr" | "ParenExpr") {
        e = kids(e).next().ok_or("clang-ast: empty cast in an enumerator")?;
    }
    let v = (kind(e) == "ConstantExpr").then(|| text(e, "value")).flatten();
    match v {
        Some("true") => Ok(1),
        Some("false") => Ok(0),
        Some(v) => v
            .parse::<i128>()
            .map_err(|_| format!("clang-ast: enumerator value `{v}` is not an integer")),
        None => Err("clang-ast: enumerator initializer without a ConstantExpr value".into()),
    }
}

fn float_literal(ty: &str, v: &str) -> Res<String> {
    let bad = || format!("clang-ast: FloatingLiteral `{v}` of type `{ty}`");
    let s = match ty {
        "float" => format!("{:e}", v.parse::<f32>().map_err(|_| bad())?),
        "double" => format!("{:e}", v.parse::<f64>().map_err(|_| bad())?),
        _ => return Err(format!("{}: only float and double are supported", bad())),
    };
    Ok(if s == "NaN" { "nan".into() } else { s })
}

fn collect_literals(n: &Json, lits: &mut Literals) {
    if matches!(kind(n), "FloatingLiteral" | "IntegerLiteral")
        && let Some(v) = text(n, "value")
    {
        lits.values.push(v.to_owned());
    }
    for c in kids(n) {
        collect_literals(c, lits);
    }
}

/// `nullptr`, `0`, or either converted (`<NullToPointer>`) or
/// parenthesized.
fn is_null_pointer_constant(n: &Json) -> bool {
    match kind(n) {
        "CXXNullPtrLiteralExpr" => true,
        "IntegerLiteral" => text(n, "value") == Some("0"),
        "ImplicitCastExpr" | "ParenExpr" | "CStyleCastExpr" => {
            kids(n).next().is_some_and(is_null_pointer_constant)
        }
        "SubstNonTypeTemplateParmExpr" => kids(n).nth(1).is_some_and(is_null_pointer_constant),
        _ => false,
    }
}
