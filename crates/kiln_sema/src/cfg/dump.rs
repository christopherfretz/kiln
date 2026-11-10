//! `-fkiln-dump-cfg`: the graph in clang's `debug.DumpCFG` layout
//! (spec/sema.md §7.5.1, spec/harness.md "cfg tests").
//!
//! The layout is clang's: a signature line, then each block as
//! ` [B<n>]` (`(ENTRY)`, `(EXIT)`, `(NORETURN)`), its label, its
//! elements as `<i>: <text>`, its terminator as `T: <text>`, and its
//! `Preds (k):`/`Succs (k):` lists, where a pruned successor prints
//! `B<n>(Unreachable)` and a dropped loop edge `NULL`. The exit is
//! `B0`, the entry the highest number and the body's blocks are
//! numbered down in program order, so a graph's numbering agrees with
//! clang's in the common cases; the harness compares shapes, not
//! numbers or element text, which is kiln's own (the expression as
//! written and its class).

use std::fmt::Write as _;

use kiln_ast::stmt::StmtKind;
use kiln_ast::{DeclId, StmtId};

use super::{Adj, BlockId, Cfg, Elem, Term, edge};
use crate::Sema;

impl Cfg {
    /// The printed number of block `b`.
    fn number(&self, b: BlockId) -> usize {
        let n = self.blocks.len();
        match b {
            BlockId::EXIT => 0,
            BlockId::ENTRY => n - 1,
            b => n - b.idx(),
        }
    }

    /// The blocks in print order: the entry, the body's, the exit.
    fn print_order(&self) -> impl Iterator<Item = BlockId> {
        let n = self.blocks.len() as u32;
        std::iter::once(BlockId::ENTRY)
            .chain((2..n).map(BlockId))
            .chain(std::iter::once(BlockId::EXIT))
    }
}

impl Sema<'_, '_> {
    /// The title of `f`'s graph in `-fkiln-dump-cfg` and
    /// `-fkiln-cfg-stats`: its qualified name, a closure type printed as
    /// `(lambda)` whatever the `-fclang-compat` level. clang's analyzer
    /// dumps no lambda call operator, so the title follows no clang
    /// release and the goldens stay compat-independent (spec/sema.md
    /// §7.5.1).
    pub(crate) fn cfg_title(&self, f: DeclId) -> String {
        use kiln_ast::types::TypeNames as _;
        let mut s = String::new();
        crate::names::sema_names(self).closures_as_lambda().write_decl_name(f, true, &mut s);
        s
    }

    /// Appends the dump of `cfg` to `out`.
    pub(crate) fn dump_cfg(&self, cfg: &Cfg, out: &mut String) {
        let name = cfg.func.map_or_else(|| "<body>".to_owned(), |f| self.cfg_title(f));
        let _ = writeln!(out, "{name}()");
        if !cfg.ok() {
            let _ = writeln!(out, " (no CFG: {:?})\n", cfg.status);
            return;
        }
        for b in cfg.print_order() {
            let blk = cfg.block(b);
            let tag = match b {
                BlockId::ENTRY => " (ENTRY)",
                BlockId::EXIT => " (EXIT)",
                _ if blk.noreturn => " (NORETURN)",
                _ => "",
            };
            let _ = writeln!(out, " [B{}{tag}]", cfg.number(b));
            if let Some(l) = blk.label {
                let _ = writeln!(out, "  {}:", self.cfg_label_text(l));
            }
            let num = cfg.number(b);
            for (i, &el) in cfg.elems(b).iter().enumerate() {
                let _ = writeln!(out, "{:>4}: {}", i + 1, self.cfg_elem_text(cfg, el));
            }
            let last = cfg.elems(b).len();
            if let Some(t) = self.cfg_term_text(blk.term, num, last) {
                let _ = writeln!(out, "   T: {t}");
            }
            if b != BlockId::ENTRY {
                let preds: Vec<&Adj> =
                    cfg.preds(b).iter().filter(|a| a.flags & edge::NULL == 0).collect();
                if !preds.is_empty() {
                    let _ = write!(out, "   Preds ({}):", preds.len());
                    for a in preds {
                        let _ = write!(out, " {}", adj_text(cfg, *a));
                    }
                    out.push('\n');
                }
            }
            if b != BlockId::EXIT {
                let succs = cfg.succs(b);
                if !succs.is_empty() {
                    let _ = write!(out, "   Succs ({}):", succs.len());
                    for a in succs {
                        if a.flags & edge::NULL != 0 {
                            out.push_str(" NULL");
                        } else {
                            let _ = write!(out, " {}", adj_text(cfg, *a));
                        }
                    }
                    out.push('\n');
                }
            }
        }
        out.push('\n');
    }

    fn cfg_label_text(&self, s: StmtId) -> String {
        let st = &self.ast.stmts;
        match st.get(s).kind() {
            StmtKind::LabelStmt => self.decl_name(st.label_decl(s)),
            StmtKind::CaseStmt => "case".to_owned(),
            StmtKind::DefaultStmt => "default".to_owned(),
            StmtKind::CXXCatchStmt => "catch".to_owned(),
            k => format!("{k:?}"),
        }
    }

    fn cfg_elem_text(&self, cfg: &Cfg, el: Elem) -> String {
        let st = &self.ast.stmts;
        match el {
            Elem::Stmt(s) => match st.get(s).kind() {
                StmtKind::DeclStmt => {
                    let list = st.decl_stmt_decls(s);
                    let names: Vec<String> = (0..self.ast.lists.len(list))
                        .filter_map(|i| self.ast.lists.at::<DeclId>(list, i))
                        .map(|d| self.decl_name(d))
                        .collect();
                    format!("decl {} (DeclStmt)", names.join(", "))
                }
                StmtKind::GotoStmt => {
                    format!("goto {} (GotoStmt)", self.decl_name(st.label_decl(s)))
                }
                k => format!("{k:?}"),
            },
            Elem::Expr(e) => {
                let mut t = self.expr_text_as_written(e);
                if t.len() > 60 {
                    let mut cut = 57;
                    while !t.is_char_boundary(cut) {
                        cut -= 1;
                    }
                    t.truncate(cut);
                    t.push_str("...");
                }
                format!("{t} ({:?})", self.ast.exprs.get(e).kind())
            }
            Elem::Init(i) => {
                let name = cfg
                    .func
                    .and_then(|f| self.ast.ctor_inits(f).nth(i as usize).map(|c| c.init))
                    .map_or_else(String::new, |e| self.expr_text_as_written(e));
                format!("{name} (Member initializer)")
            }
            Elem::AutoDtor(d) => format!("{}.~() (Implicit destructor)", self.decl_name(d)),
            Elem::TempDtor(e) => {
                format!("~() (Temporary object destructor of {})", self.expr_text_as_written(e))
            }
        }
    }

    fn cfg_term_text(&self, t: Term, num: usize, last: usize) -> Option<String> {
        let st = &self.ast.stmts;
        let r = format!("[B{num}.{last}]");
        Some(match t {
            Term::None => return None,
            Term::If(_) => format!("if {r}"),
            Term::Loop(s) => match st.get(s).kind() {
                StmtKind::WhileStmt => format!("while {r}"),
                StmtKind::DoStmt => format!("do ... while {r}"),
                StmtKind::CXXForRangeStmt => format!("for (: {r})"),
                _ => format!("for (...; {r}; ...)"),
            },
            Term::Switch(_) => format!("switch {r}"),
            Term::Try(_) => "try ...".to_owned(),
            Term::Logical(e) => {
                let or = self.ast.exprs.binary_op(e) == kiln_ast::expr::BinaryOp::LOr;
                format!("{r} {} ...", if or { "||" } else { "&&" })
            }
            Term::Cond(_) => format!("{r} ? ... : ..."),
            Term::TempDtor(e) => format!("(Temp Dtor) {}", self.expr_text_as_written(e)),
            Term::Jump(s) => match st.get(s).kind() {
                StmtKind::BreakStmt => "break;".to_owned(),
                StmtKind::ContinueStmt => "continue;".to_owned(),
                StmtKind::IndirectGotoStmt => "goto *;".to_owned(),
                _ => format!("goto {};", self.decl_name(st.label_decl(s))),
            },
            Term::Asm(_) => "asm goto".to_owned(),
            Term::IndirectGoto => "goto *;".to_owned(),
        })
    }
}

fn adj_text(cfg: &Cfg, a: Adj) -> String {
    let n = cfg.number(a.block);
    if a.flags & edge::LIVE == 0 { format!("B{n}(Unreachable)") } else { format!("B{n}") }
}
