//! The AST dump (spec/ast-dump.md): clang 19's `-ast-dump` text format,
//! deterministic and address-free, the engine behind `kiln -fdump-ast`.
//!
//! ```text
//! TranslationUnitDecl
//! |-VarDecl used x 'int' cinit
//! | `-IntegerLiteral 'int' 1
//! `-FunctionDecl f 'int ()'
//! ```
//!
//! [`dump`] writes to any [`io::Write`]; [`dump_to_string`] is the
//! in-memory form the driver calls. The traversal is iterative (no
//! recursion on the tree), so adversarially deep ASTs cannot exhaust the
//! stack, and every line is built in one reused buffer.
//!
//! The [`TypeNames`](crate::types::TypeNames) and
//! [`PrintEnv`](crate::consts::PrintEnv) implementations of [`Ast`] (and
//! of [`AstNames`], which adds a source manager) live here too: they are
//! how the dump, diagnostics and the constant printer name declarations,
//! expressions and values.

mod attrs;
mod names;

use std::fmt::Write as _;
use std::io;

use kiln_source::{FileId, Loc, SourceManager};

pub use names::AstNames;

use crate::Ast;
use crate::attr::AttrKind;
use crate::consts::PrintOptions as ValueOptions;
use crate::decl::{
    Access, ConstexprKind, DeclFlags, DeclKind, Defaulted, DefinitionBits, EnumData, FieldData,
    FnStorage, InitStyle, LinkageLang, NamespaceData, ParamData, TagKind, TlsKind, VarEvalState,
    VarStorage,
};
use crate::expr::{
    ExprDeps, ExprFlags, ExprKind, NonOdrUse, ObjectKind, StringKind, ValueCategory,
};
use crate::ids::{AttrId, DeclId, ExprId, ListId, QualType, RawId, StmtId, StringLitId};
use crate::stmt::{IfKind, StmtKind};
use crate::types::{PrintOptions, write_nns, write_type};

/// Selects the declarations [`dump`] prints as separate trees (clang's
/// `-ast-dump-filter`, spec/ast-dump.md §1). A declaration matches when
/// every given criterion holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    /// A substring of the qualified name
    /// ([`AstNames::write_qualified_name`]).
    pub name: Option<String>,
    /// The declaration kind.
    pub kind: Option<DeclKind>,
}

/// What [`dump`] prints (spec/ast-dump.md §1). The default is the
/// `-fdump-ast` default: the main file's explicit top-level declarations,
/// without ids or locations.
#[derive(Clone, Default)]
#[allow(clippy::struct_excessive_bools, reason = "independent switches, as clang's flags")]
pub struct DumpOptions<'a> {
    /// Print node ids (`#d12`, `#e5`, `#s3`, `#a2`) after class names and
    /// referenced declarations, and `prev=#dN` on redeclarations.
    pub ids: bool,
    /// Print locations (`@L:C` in the main file, `@file:L:C` elsewhere,
    /// `@<expansion>[<spelling>]` for macro locations); needs `sources`.
    pub locs: bool,
    /// Always print the type pair, its second half being the canonical
    /// type (instead of the top-level desugared type when it differs).
    pub canonical_types: bool,
    /// Include implicit declarations.
    pub implicit: bool,
    /// Include top-level declarations of files other than `main_file`.
    pub all_files: bool,
    /// Print only the matching declarations, each as its own tree.
    pub filter: Option<Filter>,
    /// The source manager the AST's locations refer to. Without it no
    /// location prints and no declaration is filtered by file.
    pub sources: Option<&'a SourceManager>,
    /// The main file: without `all_files`, top-level declarations located
    /// in other files are skipped.
    pub main_file: Option<FileId>,
}

impl std::fmt::Debug for DumpOptions<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DumpOptions")
            .field("ids", &self.ids)
            .field("locs", &self.locs)
            .field("canonical_types", &self.canonical_types)
            .field("implicit", &self.implicit)
            .field("all_files", &self.all_files)
            .field("filter", &self.filter)
            .field("sources", &self.sources.is_some())
            .field("main_file", &self.main_file)
            .finish()
    }
}

/// Writes the dump of `ast` to `out`.
///
/// # Errors
/// Only the writer's errors.
pub fn dump<W: io::Write>(ast: &Ast, opts: &DumpOptions<'_>, out: W) -> io::Result<()> {
    let mut d = Dumper::new(ast, opts, out);
    match &opts.filter {
        Some(f) => d.dump_filtered(f),
        None => d.dump_tree(Node::Decl(ast.tu)),
    }
}

/// The dump of `ast` as a string (what `kiln -fdump-ast` prints).
pub fn dump_to_string(ast: &Ast, opts: &DumpOptions<'_>) -> String {
    let mut out = Vec::new();
    // Writing to a Vec cannot fail.
    let _ = dump(ast, opts, &mut out);
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

/// A printed node.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Node {
    Decl(DeclId),
    Expr(ExprId),
    Stmt(StmtId),
    Attr(AttrId),
    /// The condition variable of an `if`/`switch`/`while`/`for`: a
    /// `DeclStmt` line with the declaration as its child (as clang).
    CondVar(DeclId),
    /// A base-specifier of a class definition (`public 'B'`).
    Base(DeclId, u32),
}

/// A pending line.
#[derive(Clone, Copy)]
struct Item {
    node: Node,
    label: Option<&'static str>,
    last: bool,
    depth: u32,
}

/// A pending node of the filter search.
#[derive(Clone, Copy)]
enum Search {
    Decl(DeclId),
    Stmt(StmtId),
}

struct Dumper<'a, W> {
    ast: &'a Ast,
    names: AstNames<'a>,
    opts: &'a DumpOptions<'a>,
    out: W,
    /// The line being built.
    line: String,
    /// The ancestors' continuation glyphs (`| ` / `  `).
    prefix: String,
    stack: Vec<Item>,
    kids: Vec<(Node, Option<&'static str>)>,
}

impl<'a, W: io::Write> Dumper<'a, W> {
    fn new(ast: &'a Ast, opts: &'a DumpOptions<'a>, out: W) -> Self {
        Dumper {
            ast,
            names: AstNames { ast, sources: opts.sources },
            opts,
            out,
            line: String::with_capacity(128),
            prefix: String::with_capacity(64),
            stack: Vec::new(),
            kids: Vec::new(),
        }
    }

    // -------------------------------------------------------------------
    // selection
    // -------------------------------------------------------------------

    /// True if member `d` of `ctx` is printed / searched.
    fn shown(&self, ctx: DeclId, d: DeclId) -> bool {
        let n = self.ast.decls.get(d);
        if !self.opts.implicit && n.has(DeclFlags::IMPLICIT) {
            return false;
        }
        if ctx == self.ast.tu
            && !self.opts.all_files
            && let (Some(sm), Some(main)) = (self.opts.sources, self.opts.main_file)
            && n.loc().is_valid()
        {
            return sm.line_col(n.loc()).is_none_or(|lc| lc.file == main);
        }
        true
    }

    fn dump_filtered(&mut self, f: &Filter) -> io::Result<()> {
        let mut todo = vec![Search::Decl(self.ast.tu)];
        let mut qual = String::new();
        while let Some(s) = todo.pop() {
            let mark = todo.len();
            match s {
                Search::Decl(d) => {
                    let n = self.ast.decls.get(d);
                    let kind = n.kind();
                    if d != self.ast.tu {
                        let kind_ok = f.kind.is_none_or(|k| k == kind);
                        let mut name_ok = true;
                        if let Some(sub) = &f.name {
                            qual.clear();
                            if !self.names.is_unnamed(d) {
                                self.names.write_qualified_name(d, &mut qual);
                            }
                            name_ok = !qual.is_empty() && qual.contains(sub.as_str());
                        }
                        if kind_ok && name_ok {
                            if f.name.is_none() {
                                qual.clear();
                                self.names.write_qualified_name(d, &mut qual);
                            }
                            writeln!(self.out, "Dumping {qual}:")?;
                            self.dump_tree(Node::Decl(d))?;
                            writeln!(self.out)?;
                            continue;
                        }
                    }
                    if kind.is_function() {
                        if let Some(fd) = self.ast.decls.function(d) {
                            for p in self.ast.lists.iter::<DeclId>(fd.params) {
                                todo.push(Search::Decl(p));
                            }
                            if let Some(b) = fd.body {
                                todo.push(Search::Stmt(b));
                            }
                        }
                    } else if self.ast.decls.member_list(d).is_some() {
                        for m in self.ast.members(d) {
                            if self.shown(d, m) {
                                todo.push(Search::Decl(m));
                            }
                        }
                    }
                }
                Search::Stmt(s) => self.search_stmt(s, &mut todo),
            }
            todo[mark..].reverse();
        }
        Ok(())
    }

    /// Pushes the declarations and statements nested in statement `s`.
    fn search_stmt(&self, s: StmtId, todo: &mut Vec<Search>) {
        let stmts = &self.ast.stmts;
        let lists = &self.ast.lists;
        let push = |s: Option<StmtId>, todo: &mut Vec<Search>| {
            if let Some(s) = s {
                todo.push(Search::Stmt(s));
            }
        };
        match stmts.get(s).kind() {
            StmtKind::CompoundStmt => {
                for c in lists.iter::<StmtId>(stmts.compound_body(s)) {
                    push(Some(c), todo);
                }
            }
            StmtKind::DeclStmt => {
                for d in lists.iter::<DeclId>(stmts.decl_stmt_decls(s)) {
                    if self.opts.implicit || !names::is_implicit(self.ast, d) {
                        todo.push(Search::Decl(d));
                    }
                }
            }
            StmtKind::IfStmt => {
                let i = *stmts.if_data(s);
                push(i.init, todo);
                if let Some(v) = i.cond_var {
                    todo.push(Search::Decl(v));
                }
                push(i.then, todo);
                push(i.els, todo);
            }
            StmtKind::SwitchStmt => {
                let w = *stmts.switch_data(s);
                push(w.init, todo);
                if let Some(v) = w.cond_var {
                    todo.push(Search::Decl(v));
                }
                push(Some(w.body), todo);
            }
            StmtKind::CaseStmt => push(Some(stmts.case_data(s).sub), todo),
            StmtKind::DefaultStmt => push(Some(stmts.default_sub(s)), todo),
            StmtKind::WhileStmt | StmtKind::ForStmt => {
                let l = *stmts.loop_data(s);
                push(l.init, todo);
                if let Some(v) = l.cond_var {
                    todo.push(Search::Decl(v));
                }
                push(l.body, todo);
            }
            StmtKind::DoStmt => push(stmts.loop_data(s).body, todo),
            _ => {}
        }
    }

    // -------------------------------------------------------------------
    // the tree
    // -------------------------------------------------------------------

    fn dump_tree(&mut self, root: Node) -> io::Result<()> {
        self.prefix.clear();
        self.stack.clear();
        self.stack.push(Item { node: root, label: None, last: true, depth: 0 });
        while let Some(it) = self.stack.pop() {
            let depth = it.depth as usize;
            self.line.clear();
            if depth > 0 {
                self.prefix.truncate(2 * (depth - 1));
                self.line.push_str(&self.prefix);
                self.line.push_str(if it.last { "`-" } else { "|-" });
            }
            if let Some(l) = it.label {
                self.line.push_str(l);
                self.line.push_str(": ");
            }
            match it.node {
                Node::Decl(d) => self.decl_line(d),
                Node::Expr(e) => self.expr_line(e),
                Node::Stmt(s) => self.stmt_line(s),
                Node::Attr(a) => self.attr_line(a),
                Node::CondVar(_) => self.line.push_str("DeclStmt"),
                Node::Base(r, i) => self.base_line(r, i),
            }
            debug_assert!(!self.line.ends_with(' '), "trailing whitespace: {:?}", self.line);
            self.line.push('\n');
            self.out.write_all(self.line.as_bytes())?;
            if depth > 0 {
                self.prefix.push_str(if it.last { "  " } else { "| " });
            }
            let mut kids = std::mem::take(&mut self.kids);
            kids.clear();
            self.children(it.node, &mut kids);
            let n = kids.len();
            for (i, &(node, label)) in kids.iter().enumerate().rev() {
                self.stack.push(Item { node, label, last: i + 1 == n, depth: it.depth + 1 });
            }
            self.kids = kids;
        }
        Ok(())
    }

    /// The node printed for statement `s` (`ExprStmt` is elided).
    fn stmt_node(&self, s: StmtId) -> Node {
        if self.ast.stmts.get(s).kind() == StmtKind::ExprStmt {
            Node::Expr(self.ast.stmts.expr_stmt_expr(s))
        } else {
            Node::Stmt(s)
        }
    }

    fn children(&self, node: Node, kids: &mut Vec<(Node, Option<&'static str>)>) {
        match node {
            Node::Decl(d) => self.decl_children(d, kids),
            Node::Expr(e) => self.expr_children(e, kids),
            Node::Stmt(s) => self.stmt_children(s, kids),
            Node::CondVar(d) => kids.push((Node::Decl(d), None)),
            Node::Attr(_) | Node::Base(..) => {}
        }
    }

    fn decl_children(&self, d: DeclId, kids: &mut Vec<(Node, Option<&'static str>)>) {
        let ast = self.ast;
        let decls = &ast.decls;
        let kind = decls.get(d).kind();
        let mut expr = |e: Option<ExprId>| {
            if let Some(e) = e {
                kids.push((Node::Expr(e), None));
            }
        };
        if kind.is_function() {
            if let Some(f) = decls.function(d) {
                for p in ast.lists.iter::<DeclId>(f.params) {
                    kids.push((Node::Decl(p), None));
                }
                if let Some(b) = f.body {
                    kids.push((self.stmt_node(b), None));
                }
            }
        } else if let Some(v) = decls.var(d) {
            expr(v.init);
        } else if let Some(p) = decls.param(d) {
            expr(p.default_arg);
        } else if let Some(f) = decls.field(d) {
            expr(f.bit_width);
            expr(f.init);
        } else if let Some(e) = decls.enumerator(d) {
            expr(e.init);
        } else if let Some(s) = decls.static_assert(d) {
            expr(Some(s.cond));
            expr(s.message);
        } else if decls.record(d).is_some() {
            for i in 0..decls.bases(d, &ast.lists).count() {
                kids.push((Node::Base(d, i as u32), None));
            }
        }
        for a in ast.lists.iter::<AttrId>(ast.attrs.of_decl(d)) {
            if self.opts.implicit || ast.attrs.get(a).flags & 1 == 0 {
                kids.push((Node::Attr(a), None));
            }
        }
        if !kind.is_function() && decls.member_list(d).is_some() {
            for m in ast.members(d) {
                if self.shown(d, m) {
                    kids.push((Node::Decl(m), None));
                }
            }
        }
    }

    fn expr_children(&self, e: ExprId, kids: &mut Vec<(Node, Option<&'static str>)>) {
        let exprs = &self.ast.exprs;
        let lists = &self.ast.lists;
        let n = exprs.get(e);
        let mut push = |w: u32| {
            if let Some(c) = ExprId::from_raw(w) {
                kids.push((Node::Expr(c), None));
            }
        };
        match n.kind() {
            ExprKind::MemberExpr
            | ExprKind::UnaryOperator
            | ExprKind::ParenExpr
            | ExprKind::ConstantExpr => push(n.a()),
            k if k.is_cast() => push(n.a()),
            ExprKind::UnaryExprOrTypeTraitExpr => {
                if !n.has(ExprFlags::ARG_IS_TYPE) {
                    push(n.a());
                }
            }
            ExprKind::CXXNoexceptExpr => push(n.a()),
            ExprKind::BinaryOperator | ExprKind::ArraySubscriptExpr => {
                push(n.a());
                push(n.b());
            }
            ExprKind::CompoundAssignOperator => {
                push(n.a());
                push(exprs.compound_assign(e).rhs.raw());
            }
            ExprKind::ConditionalOperator => {
                push(n.a());
                for c in exprs.cond_arms(e) {
                    push(c.raw());
                }
            }
            ExprKind::CallExpr => {
                push(n.a());
                for c in lists.iter::<ExprId>(exprs.call_args(e)) {
                    push(c.raw());
                }
            }
            ExprKind::InitListExpr => {
                if let Some(f) = exprs.init_list(e).array_filler {
                    kids.push((Node::Expr(f), Some("array_filler")));
                }
                for c in lists.iter::<ExprId>(exprs.init_list_inits(e)) {
                    kids.push((Node::Expr(c), None));
                }
            }
            ExprKind::RecoveryExpr => {
                if let Some(l) = ListId::from_raw(n.a()) {
                    for c in lists.iter::<ExprId>(l) {
                        kids.push((Node::Expr(c), None));
                    }
                }
            }
            _ => {}
        }
    }

    fn stmt_children(&self, s: StmtId, kids: &mut Vec<(Node, Option<&'static str>)>) {
        let stmts = &self.ast.stmts;
        let lists = &self.ast.lists;
        let stmt = |s: Option<StmtId>, kids: &mut Vec<(Node, Option<&'static str>)>| {
            if let Some(s) = s {
                kids.push((self.stmt_node(s), None));
            }
        };
        let expr = |e: Option<ExprId>, kids: &mut Vec<(Node, Option<&'static str>)>| {
            if let Some(e) = e {
                kids.push((Node::Expr(e), None));
            }
        };
        let var = |d: Option<DeclId>, kids: &mut Vec<(Node, Option<&'static str>)>| {
            if let Some(d) = d {
                kids.push((Node::CondVar(d), None));
            }
        };
        match stmts.get(s).kind() {
            StmtKind::CompoundStmt => {
                for c in lists.iter::<StmtId>(stmts.compound_body(s)) {
                    stmt(Some(c), kids);
                }
            }
            StmtKind::DeclStmt => {
                for d in lists.iter::<DeclId>(stmts.decl_stmt_decls(s)) {
                    if self.opts.implicit || !names::is_implicit(self.ast, d) {
                        kids.push((Node::Decl(d), None));
                    }
                }
            }
            StmtKind::ExprStmt => expr(Some(stmts.expr_stmt_expr(s)), kids),
            StmtKind::IfStmt => {
                let i = *stmts.if_data(s);
                stmt(i.init, kids);
                var(i.cond_var, kids);
                expr(i.cond, kids);
                stmt(i.then, kids);
                stmt(i.els, kids);
            }
            StmtKind::SwitchStmt => {
                let w = *stmts.switch_data(s);
                stmt(w.init, kids);
                var(w.cond_var, kids);
                expr(Some(w.cond), kids);
                stmt(Some(w.body), kids);
            }
            StmtKind::CaseStmt => {
                let c = *stmts.case_data(s);
                expr(Some(c.lhs), kids);
                expr(c.rhs, kids);
                stmt(Some(c.sub), kids);
            }
            StmtKind::DefaultStmt => stmt(Some(stmts.default_sub(s)), kids),
            StmtKind::WhileStmt | StmtKind::ForStmt => {
                let l = *stmts.loop_data(s);
                stmt(l.init, kids);
                var(l.cond_var, kids);
                expr(l.cond, kids);
                expr(l.inc, kids);
                stmt(l.body, kids);
            }
            StmtKind::DoStmt => {
                let l = *stmts.loop_data(s);
                stmt(l.body, kids);
                expr(l.cond, kids);
            }
            StmtKind::ReturnStmt => expr(stmts.return_value(s), kids),
            _ => {}
        }
    }

    // -------------------------------------------------------------------
    // line pieces
    // -------------------------------------------------------------------

    fn id(&mut self, sigil: char, raw: u32) {
        if self.opts.ids {
            let _ = write!(self.line, " #{sigil}{raw}");
        }
    }

    fn loc(&mut self, loc: Loc) {
        if !self.opts.locs || !loc.is_valid() {
            return;
        }
        let Some(sm) = self.opts.sources else { return };
        if sm.is_macro_loc(loc) {
            self.line.push_str(" @");
            self.plain_loc(sm, sm.expansion_loc(loc));
            self.line.push('[');
            self.plain_loc(sm, sm.spelling_loc(loc));
            self.line.push(']');
        } else {
            self.line.push_str(" @");
            self.plain_loc(sm, loc);
        }
    }

    /// `L:C` in the main file, `file:L:C` elsewhere, `<invalid>` if the
    /// location cannot be resolved.
    fn plain_loc(&mut self, sm: &SourceManager, loc: Loc) {
        let Some(p) = sm.presumed_line_col(loc) else {
            self.line.push_str("<invalid>");
            return;
        };
        let main =
            self.opts.main_file.is_some_and(|m| p.file == m && p.file_name == sm.file_name(m));
        if !main {
            self.line.push_str(p.file_name);
            self.line.push(':');
        }
        let _ = write!(self.line, "{}:{}", p.line, p.col);
    }

    /// ` 'type'[:'desugared']` (spec/ast-dump.md §2).
    fn ty(&mut self, q: QualType) {
        self.line.push(' ');
        self.bare_ty(q);
    }

    fn bare_ty(&mut self, q: QualType) {
        let types = &self.ast.types;
        self.line.push('\'');
        let s1 = self.line.len();
        write_type(types, &self.names, q, PrintOptions::SUGARED, &mut self.line);
        let e1 = self.line.len();
        self.line.push_str("':'");
        let s2 = self.line.len();
        if self.opts.canonical_types {
            write_type(types, &self.names, q, PrintOptions::CANONICAL, &mut self.line);
        } else {
            write_type(types, &self.names, q, PrintOptions::DESUGARED, &mut self.line);
            if self.line[s2..] == self.line[s1..e1] {
                self.line.truncate(e1 + 1);
                return;
            }
        }
        self.line.push('\'');
    }

    fn value(&mut self, v: crate::ids::ConstValueId) {
        self.line.push_str(" value=");
        self.ast.consts.print(
            v,
            &self.ast.strings,
            &self.names,
            &ValueOptions::DEFAULT,
            &mut self.line,
        );
    }

    /// clang's `dumpBareDeclRef`: `Kind [#dN] 'name' ['type']`.
    fn decl_ref(&mut self, d: DeclId) {
        let n = self.ast.decls.get(d);
        self.line.push_str(n.kind().name());
        self.id('d', d.raw());
        self.line.push_str(" '");
        self.names.write_name(d, &mut self.line);
        self.line.push('\'');
        if is_value_decl(n.kind())
            && let Some(t) = n.ty()
        {
            self.ty(t);
        }
    }

    // -------------------------------------------------------------------
    // declarations
    // -------------------------------------------------------------------

    fn decl_line(&mut self, d: DeclId) {
        let ast = self.ast;
        let decls = &ast.decls;
        let n = *decls.get(d);
        let kind = n.kind();
        self.line.push_str(kind.name());
        self.line.push_str("Decl");
        self.id('d', d.raw());
        self.loc(n.loc());
        if self.opts.ids
            && let Some(p) = ast.prev(d)
        {
            let _ = write!(self.line, " prev=#d{}", p.raw());
        }
        if n.has(DeclFlags::IMPLICIT) {
            self.line.push_str(" implicit");
        }
        // clang: `isUsed` reads the first declaration,
        // `isThisDeclarationReferenced` only this one.
        if n.has(DeclFlags::USED) || decls.get(ast.first(d)).has(DeclFlags::USED) {
            self.line.push_str(" used");
        } else if n.has(DeclFlags::REFERENCED) {
            self.line.push_str(" referenced");
        }
        if n.has(DeclFlags::INVALID) {
            self.line.push_str(" invalid");
        }
        if kind.is_function() {
            self.function_fields(d);
        } else if let Some(v) = decls.var(d) {
            let v = *v;
            self.name(d);
            self.opt_ty(n.ty());
            let spec = v.spec;
            self.line.push_str(match spec.storage() {
                VarStorage::None => "",
                VarStorage::Static => " static",
                VarStorage::Extern => " extern",
                VarStorage::Register => " register",
            });
            self.line.push_str(match spec.tls() {
                TlsKind::None => "",
                TlsKind::Static => " tls",
                TlsKind::Dynamic => " tls_dynamic",
            });
            for (on, word) in [
                (spec.is_nrvo(), " nrvo"),
                (spec.is_inline(), " inline"),
                (spec.is_constexpr(), " constexpr"),
            ] {
                if on {
                    self.line.push_str(word);
                }
            }
            if v.init.is_some() {
                self.line.push_str(match spec.init_style() {
                    InitStyle::C => " cinit",
                    InitStyle::Call => " callinit",
                    InitStyle::List => " listinit",
                    InitStyle::ParenList => " parenlistinit",
                });
                if spec.is_constexpr()
                    && let VarEvalState::Value(c) = v.eval.state()
                {
                    self.value(c);
                }
            }
        } else if let Some(p) = decls.param(d) {
            let p = *p;
            self.name(d);
            self.opt_ty(n.ty());
            if p.default_arg.is_some() {
                self.line.push_str(" cinit");
            }
            if p.flags & ParamData::PACK != 0 {
                self.line.push_str(" pack");
            }
        } else {
            self.other_fields(d, &n);
        }
    }

    /// ` name` if `d` is named.
    fn name(&mut self, d: DeclId) {
        let mark = self.line.len();
        self.line.push(' ');
        self.names.write_name(d, &mut self.line);
        if self.line.len() == mark + 1 {
            self.line.truncate(mark);
        }
    }

    fn opt_ty(&mut self, q: Option<QualType>) {
        if let Some(q) = q {
            self.ty(q);
        }
    }

    fn function_fields(&mut self, d: DeclId) {
        let ast = self.ast;
        let n = *ast.decls.get(d);
        let Some(f) = ast.decls.function(d) else {
            self.name(d);
            self.opt_ty(n.ty());
            return;
        };
        let spec = f.spec;
        match spec.constexpr_kind() {
            ConstexprKind::Constexpr => self.line.push_str(" constexpr"),
            ConstexprKind::Consteval => self.line.push_str(" consteval"),
            ConstexprKind::None if spec.escalated() => self.line.push_str(" immediate"),
            ConstexprKind::None => {}
        }
        self.name(d);
        self.opt_ty(n.ty());
        self.line.push_str(match spec.storage() {
            FnStorage::None => "",
            FnStorage::Static => " static",
            FnStorage::Extern => " extern",
        });
        let defaulted = spec.defaulted() != Defaulted::No;
        for (on, word) in [
            (spec.is_inline(), " inline"),
            (spec.is_virtual(), " virtual"),
            (spec.is_pure(), " pure"),
            (defaulted, " default"),
            (defaulted && spec.is_deleted(), "_delete"),
            (!defaulted && spec.is_deleted(), " delete"),
            (spec.is_trivial(), " trivial"),
        ] {
            if on {
                self.line.push_str(word);
            }
        }
        if !spec.is_inline() && self.implicitly_inline(d) {
            self.line.push_str(" implicit-inline");
        }
    }

    /// clang's `isInlined()` without the specifier: constexpr, consteval,
    /// deleted, defined in its class (`implicit_inline`), or a
    /// redeclaration of an inline function.
    fn implicitly_inline(&self, d: DeclId) -> bool {
        let decls = &self.ast.decls;
        let mut cur = Some(d);
        for _ in 0..1024 {
            let Some(c) = cur else { return false };
            if let Some(f) = decls.function(c) {
                let s = f.spec;
                if s.implicit_inline()
                    || s.constexpr_kind() != ConstexprKind::None
                    || s.is_deleted()
                    || (c != d && s.is_inline())
                {
                    return true;
                }
            }
            cur = self.ast.prev(c);
        }
        false
    }

    fn other_fields(&mut self, d: DeclId, n: &crate::decl::DeclNode) {
        let ast = self.ast;
        let decls = &ast.decls;
        match n.kind() {
            DeclKind::TranslationUnit | DeclKind::StaticAssert | DeclKind::Empty => {}
            DeclKind::Typedef | DeclKind::TypeAlias => {
                self.name(d);
                let under = decls.typedef(d).map(|t| t.underlying).or(n.ty());
                self.opt_ty(under);
            }
            DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization => {
                if let Some(r) = decls.record(d) {
                    let (tag, defn) = (r.tag, r.defn);
                    self.line.push(' ');
                    self.line.push_str(tag.keyword());
                    self.name(d);
                    if defn.contains(DefinitionBits::COMPLETE)
                        || defn.contains(DefinitionBits::BEING_DEFINED)
                    {
                        self.line.push_str(" definition");
                    }
                } else {
                    self.name(d);
                }
            }
            DeclKind::Field => {
                self.name(d);
                self.opt_ty(n.ty());
                if decls.field(d).is_some_and(|f| f.flags & FieldData::MUTABLE != 0) {
                    self.line.push_str(" mutable");
                }
            }
            DeclKind::Enum => {
                let e = decls.enumeration(d).copied();
                let flags = e.map_or(0, |e| e.flags);
                if flags & EnumData::SCOPED != 0 {
                    self.line.push_str(if flags & EnumData::SCOPED_STRUCT != 0 {
                        " struct"
                    } else {
                        " class"
                    });
                }
                self.name(d);
                if flags & (EnumData::SCOPED | EnumData::FIXED) != 0 {
                    self.opt_ty(e.and_then(|e| e.int_type));
                }
            }
            DeclKind::EnumConstant => {
                self.name(d);
                self.opt_ty(n.ty());
                if let Some(v) = decls.enumerator(d).and_then(|e| e.value) {
                    self.value(v);
                }
            }
            DeclKind::Namespace => {
                self.name(d);
                if decls.namespace(d).is_some_and(|ns| ns.flags & NamespaceData::INLINE != 0) {
                    self.line.push_str(" inline");
                }
                let first = ast.first(d);
                if first != d {
                    self.line.push_str(" original ");
                    self.decl_ref(first);
                }
            }
            DeclKind::Using
            | DeclKind::NamespaceAlias
            | DeclKind::UsingShadow
            | DeclKind::UsingEnum
            | DeclKind::UsingDirective => self.using_fields(d, n.kind()),
            DeclKind::LinkageSpec => {
                let lang = decls.scope(d).map_or(0, |s| s.aux);
                self.line.push_str(if lang == LinkageLang::C as u32 { " C" } else { " C++" });
            }
            DeclKind::AccessSpec => {
                self.line.push_str(match n.misc().access() {
                    Access::Public => " public",
                    Access::Protected => " protected",
                    Access::Private => " private",
                    Access::None => "",
                });
            }
            _ => {
                self.name(d);
                if is_value_decl(n.kind()) {
                    self.opt_ty(n.ty());
                }
            }
        }
    }

    /// The fields of a using-family declaration (spec/ast-dump.md §3).
    fn using_fields(&mut self, d: DeclId, kind: DeclKind) {
        let ast = self.ast;
        match kind {
            DeclKind::Using => {
                self.line.push(' ');
                if let Some(q) = ast.using_qualifier(d) {
                    write_nns(&ast.types, &self.names, q, &mut self.line);
                }
                self.names.write_name(d, &mut self.line);
                return;
            }
            DeclKind::NamespaceAlias => self.name(d),
            _ => {}
        }
        if let Some(t) = ast.using_target(d) {
            self.line.push(' ');
            self.decl_ref(t);
        }
    }

    fn base_line(&mut self, r: DeclId, i: u32) {
        let Some(b) = self.ast.decls.bases(r, &self.ast.lists).nth(i as usize).copied() else {
            return;
        };
        if b.is_virtual {
            self.line.push_str("virtual ");
        }
        // clang prints the semantic access: a defaulted one is `private`
        // in a `class`, `public` otherwise ([class.access.base]/2).
        let class = self.ast.decls.record(r).is_some_and(|r| r.tag == TagKind::Class);
        self.line.push_str(match b.access {
            Access::Protected => "protected ",
            Access::Private => "private ",
            Access::None if class => "private ",
            Access::Public | Access::None => "public ",
        });
        self.bare_ty(b.ty);
        if b.pack_expansion {
            self.line.push_str("...");
        }
    }

    fn attr_line(&mut self, a: AttrId) {
        let node = *self.ast.attrs.get(a);
        if let Some(name) = attrs::class_name(&node) {
            self.line.push_str(name);
            self.line.push_str("Attr");
            self.id('a', a.raw());
        } else {
            self.line.push_str("UnknownAttr");
            self.id('a', a.raw());
            let _ = write!(self.line, " kind={}", node.kind.0);
        }
        self.loc(node.loc);
        if node.flags & 2 != 0 {
            self.line.push_str(" Inherited");
        }
        if node.flags & 1 != 0 {
            self.line.push_str(" Implicit");
        }
        if node.kind == AttrKind::ASM_LABEL
            && let Some(l) = StringLitId::from_raw(node.a)
        {
            // clang prints the label's bytes unescaped.
            self.line.push_str(" \"");
            self.line.push_str(&String::from_utf8_lossy(self.ast.strings.bytes(l)));
            self.line.push_str("\" IsLiteralLabel");
        }
        if node.kind == AttrKind::BUILTIN {
            let _ = write!(self.line, " {}", node.a);
        }
    }

    // -------------------------------------------------------------------
    // statements
    // -------------------------------------------------------------------

    fn stmt_line(&mut self, s: StmtId) {
        let stmts = &self.ast.stmts;
        let n = *stmts.get(s);
        let _ = write!(self.line, "{:?}", n.kind());
        self.id('s', s.raw());
        self.loc(n.loc());
        match n.kind() {
            StmtKind::IfStmt => {
                let i = *stmts.if_data(s);
                self.flags(&[
                    (i.init.is_some(), " has_init"),
                    (i.cond_var.is_some(), " has_var"),
                    (i.els.is_some(), " has_else"),
                ]);
                self.line.push_str(match stmts.if_kind(s) {
                    IfKind::Ordinary => "",
                    IfKind::Constexpr => " constexpr",
                    IfKind::Consteval => " consteval",
                    IfKind::NotConsteval => " !consteval",
                });
            }
            StmtKind::SwitchStmt => {
                let w = *stmts.switch_data(s);
                self.flags(&[(w.init.is_some(), " has_init"), (w.cond_var.is_some(), " has_var")]);
            }
            StmtKind::CaseStmt => {
                if stmts.case_data(s).rhs.is_some() {
                    self.line.push_str(" gnu_range");
                }
            }
            StmtKind::WhileStmt => {
                let l = *stmts.loop_data(s);
                self.flags(&[(l.cond_var.is_some(), " has_var")]);
            }
            StmtKind::ForStmt => {
                let l = *stmts.loop_data(s);
                self.flags(&[
                    (l.init.is_some(), " has_init"),
                    (l.cond_var.is_some(), " has_var"),
                    (l.cond.is_some(), " has_cond"),
                    (l.inc.is_some(), " has_inc"),
                ]);
            }
            StmtKind::ReturnStmt => {
                if let Some(v) = stmts.return_nrvo(s) {
                    self.line.push_str(" nrvo_candidate(");
                    self.decl_ref(v);
                    self.line.push(')');
                }
            }
            _ => {}
        }
    }

    fn flags(&mut self, flags: &[(bool, &str)]) {
        for &(on, word) in flags {
            if on {
                self.line.push_str(word);
            }
        }
    }

    // -------------------------------------------------------------------
    // expressions
    // -------------------------------------------------------------------

    #[allow(
        clippy::too_many_lines,
        reason = "one arm per expression kind, as clang's TextNodeDumper"
    )]
    fn expr_line(&mut self, e: ExprId) {
        let ast = self.ast;
        let exprs = &ast.exprs;
        let n = *exprs.get(e);
        let kind = n.kind();
        let _ = write!(self.line, "{kind:?}");
        self.id('e', e.raw());
        self.loc(n.loc());
        if kind == ExprKind::StringLiteral && exprs.string_kind(e) == StringKind::Unevaluated {
            self.line.push_str(" unevaluated ");
            ast.strings.write_literal(exprs.string_lit(e), &mut self.line);
            return;
        }
        self.ty(n.ty());
        if n.deps().contains(ExprDeps::ERROR) {
            self.line.push_str(" contains-errors");
        }
        match n.value_category() {
            ValueCategory::PRValue => {}
            ValueCategory::LValue => self.line.push_str(" lvalue"),
            ValueCategory::XValue => self.line.push_str(" xvalue"),
        }
        match n.object_kind() {
            ObjectKind::Ordinary => {}
            ObjectKind::BitField => self.line.push_str(" bitfield"),
            ObjectKind::VectorComponent => self.line.push_str(" vectorcomponent"),
        }
        match kind {
            ExprKind::IntegerLiteral => match exprs.int_value(e) {
                Some(v) => {
                    let _ = write!(self.line, " {v}");
                }
                None => {
                    if let Some(c) = exprs.literal_const(e) {
                        self.line.push(' ');
                        self.names.write_dump_value(c, &mut self.line);
                    }
                }
            },
            ExprKind::CharacterLiteral => {
                let _ = write!(self.line, " {}", exprs.char_value(e));
            }
            ExprKind::FloatingLiteral => {
                if let Some(c) = exprs.literal_const(e) {
                    self.line.push(' ');
                    self.names.write_dump_value(c, &mut self.line);
                }
            }
            ExprKind::StringLiteral => {
                self.line.push(' ');
                ast.strings.write_literal(exprs.string_lit(e), &mut self.line);
            }
            ExprKind::CXXBoolLiteralExpr => {
                self.line.push_str(if exprs.bool_value(e) { " true" } else { " false" });
            }
            ExprKind::DeclRefExpr => {
                self.line.push(' ');
                self.decl_ref(exprs.decl(e));
                if let Some(found) = exprs.decl_ref_extra(e).and_then(|x| x.found_decl) {
                    self.line.push_str(" (");
                    self.decl_ref(found);
                    self.line.push(')');
                }
                if n.has(ExprFlags::REFERS_TO_ENCLOSING) {
                    self.line.push_str(" refers_to_enclosing_variable_or_capture");
                }
                self.non_odr_use(exprs.non_odr_use(e));
            }
            ExprKind::MemberExpr => {
                self.line.push_str(if n.has(ExprFlags::ARROW) { " ->" } else { " ." });
                let m = exprs.decl(e);
                self.names.write_name(m, &mut self.line);
                self.id('d', m.raw());
                self.non_odr_use(exprs.non_odr_use(e));
            }
            ExprKind::CallExpr => {
                if n.has(ExprFlags::ADL) {
                    self.line.push_str(" adl");
                }
            }
            ExprKind::UnaryOperator => {
                let op = exprs.unary_op(e);
                self.line.push_str(if op.is_postfix() { " postfix '" } else { " prefix '" });
                self.line.push_str(op.spelling());
                self.line.push('\'');
                if n.has(ExprFlags::CANNOT_OVERFLOW) {
                    self.line.push_str(" cannot overflow");
                }
            }
            ExprKind::BinaryOperator => {
                let _ = write!(self.line, " '{}'", exprs.binary_op(e).spelling());
            }
            ExprKind::CompoundAssignOperator => {
                let _ = write!(self.line, " '{}' ComputeLHSTy=", exprs.binary_op(e).spelling());
                let x = *exprs.compound_assign(e);
                self.bare_ty(x.lhs_ty);
                self.line.push_str(" ComputeResultTy=");
                self.bare_ty(x.result_ty);
            }
            k if k.is_cast() => {
                let _ = write!(self.line, " <{}>", exprs.cast_kind(e).spelling());
                if k == ExprKind::ImplicitCastExpr && n.has(ExprFlags::PART_OF_EXPLICIT_CAST) {
                    self.line.push_str(" part_of_explicit_cast");
                }
            }
            ExprKind::UnaryExprOrTypeTraitExpr => {
                self.line.push(' ');
                self.line.push_str(exprs.trait_kind(e).spelling());
                if let Some(t) = exprs.trait_type_arg(e) {
                    self.ty(t);
                }
            }
            ExprKind::InitListExpr => {
                if let Some(f) = exprs.init_list(e).union_field {
                    self.line.push_str(" field=");
                    self.names.write_name(f, &mut self.line);
                    self.id('d', f.raw());
                }
            }
            ExprKind::ConstantExpr => {
                if let Some(v) = exprs.constant_value(e) {
                    self.value(v);
                }
            }
            _ => {}
        }
    }

    fn non_odr_use(&mut self, u: NonOdrUse) {
        if u != NonOdrUse::None {
            self.line.push(' ');
            self.line.push_str(u.spelling());
        }
    }
}

/// clang's `ValueDecl`s: the declarations whose type a reference prints.
fn is_value_decl(k: DeclKind) -> bool {
    use crate::decl::PayloadClass as P;
    matches!(k.payload(), P::Function | P::Var | P::Field | P::Enumerator | P::Param)
        || matches!(
            k,
            DeclKind::Binding
                | DeclKind::NonTypeTemplateParm
                | DeclKind::IndirectField
                | DeclKind::TemplateParamObject
        )
}
