//! Unannotated fall-through between switch labels (WP-217; clang
//! `DiagnoseSwitchLabelsFallthrough` / `FallthroughMapper::
//! checkFallThroughIntoBlock`, spec/sema.md "Statement attributes"):
//! `-Wimplicit-fallthrough` at a case or default label that control can
//! fall into from a statement that is not `[[fallthrough]]`, and
//! `-Wunreachable-code-fallthrough` at a `[[fallthrough]]` in unreachable
//! code that falls into a label.
//!
//! The predecessors of a label are found structurally (the statements
//! before it in its block, the branches of an `if` ending there), their
//! reachability from the body's graph ([`crate::cfg`], `stmt_flow`:
//! where control enters and leaves each statement, every case label
//! reachable, clang `fillReachableBlocks`; spec/sema.md §7.5.1).

use std::collections::HashMap;

use kiln_ast::StmtId;
use kiln_ast::attr::AttrKind;
use kiln_ast::stmt::StmtKind;
use kiln_diag::{DiagId, FixIt};
use kiln_lex::Std;
use kiln_source::{Loc, SourceManager};

use crate::Sema;

/// One definition of an object-like macro whose replacement list starts
/// `[ [`, as recorded by the preprocessor (`kiln_pp::AttrMacro`; the
/// parser copies the history in with [`Sema::sync_attr_macros`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttrMacro {
    /// The macro name.
    pub name: Box<str>,
    /// Where the definition took effect.
    pub def: Loc,
    /// Where it stopped being in effect; invalid while it is in effect.
    pub end: Loc,
    /// The replacement list's token spellings joined by single spaces.
    pub tokens: Box<str>,
}

/// `[[fallthrough]]` and `[[clang::fallthrough]]` as token lists.
const STD_TOKENS: &str = "[ [ fallthrough ] ]";
const CLANG_TOKENS: &str = "[ [ clang :: fallthrough ] ]";

/// What precedes a statement in the body: its parent and its index
/// there (a compound statement's child), or `None` for the body.
type Parents = HashMap<StmtId, (StmtId, usize)>;

/// The statements of each compound statement holding a label, collected
/// once (a label's walk back reads its block's statements; rebuilding
/// them per label made a large `switch` quadratic).
type Kids = HashMap<StmtId, Vec<StmtId>>;

/// The predecessors falling into one label.
#[derive(Default)]
struct Preds {
    /// Reachable predecessors not ending in `[[fallthrough]]`.
    unannotated: u32,
    /// Predecessors ending in `[[fallthrough]]` (or holding one, when
    /// unreachable).
    annotated: u32,
    /// The `[[fallthrough]]` statements of unreachable predecessors.
    dead: Vec<StmtId>,
}

/// Whether the end of a statement adds a predecessor (`Done`) or the
/// search goes on before it (`Empty`: it holds no CFG element).
#[derive(PartialEq, Eq)]
enum End {
    Done,
    Empty,
}

/// Reachability before and after each statement.
struct Flow {
    before: HashMap<StmtId, bool>,
    after: HashMap<StmtId, bool>,
    /// Per `do` statement holding a label: its condition is reached and
    /// loops back to the body (clang's loop-back block is reachable).
    do_back: HashMap<StmtId, bool>,
}

impl Flow {
    fn before(&self, s: StmtId) -> bool {
        self.before.get(&s).copied().unwrap_or(false)
    }

    fn after(&self, s: StmtId) -> bool {
        self.after.get(&s).copied().unwrap_or(false)
    }
}

impl Sema<'_, '_> {
    /// Records the language standard (by the parser).
    pub fn set_std(&mut self, std: Std) {
        self.std = std;
    }

    /// Whether `char8_t` exists and a `u8` literal has it as its type
    /// ([lex.ccon]/3, [lex.string]/6; clang `LangOptions::Char8`):
    /// [`Sema::char8`], else from C++20 on.
    #[must_use]
    pub fn char8(&self) -> bool {
        self.char8.unwrap_or(self.std >= Std::Cxx20)
    }

    /// `-Wimplicit-fallthrough` and `-Wunreachable-code-fallthrough` over
    /// the switch labels of function body `body`, in source order (clang
    /// visits the CFG blocks in order). Not in an instantiation for the
    /// unreachable annotation (it may be reachable in another one).
    pub(crate) fn check_implicit_fallthrough(&mut self, body: StmtId) {
        let in_inst = !self.inst.stack.is_empty();
        let warn = self.diags.is_enabled(DiagId::sema_implicit_fallthrough);
        let dead_on = !in_inst && self.diags.is_enabled(DiagId::sema_unreachable_fallthrough_attr);
        if !warn && !dead_on {
            return;
        }
        let mut labels = Vec::new();
        let mut parents = Parents::new();
        let mut kids = Kids::new();
        let mut switch_bodies = Vec::new();
        self.ift_collect(body, &mut labels, &mut parents, &mut kids, &mut switch_bodies);
        if labels.is_empty() {
            return;
        }
        let Some((before, after)) = self.with_cfg(body, |_, c| c.stmt_flow()) else { return };
        let mut flow = Flow { before, after, do_back: HashMap::new() };
        let dos: Vec<StmtId> = parents
            .iter()
            .filter(|&(&c, &(p, _))| {
                self.ast.stmts.get(p).kind() == StmtKind::DoStmt
                    && self.ast.stmts.loop_data(p).body == Some(c)
            })
            .map(|(_, &(p, _))| p)
            .collect();
        for d in dos {
            let back = self.ift_do_back(d, &flow);
            flow.do_back.insert(d, back);
        }
        for l in labels {
            let mut into = Preds::default();
            self.ift_label_preds(l, &parents, &kids, &switch_bodies, &flow, &mut into);
            if dead_on {
                for s in std::mem::take(&mut into.dead) {
                    let loc = self.ast.stmts.get(s).loc();
                    let d = self.diag(DiagId::sema_unreachable_fallthrough_attr, loc);
                    self.emit(d);
                }
            }
            if into.unannotated == 0 || !warn {
                continue;
            }
            let at = self.ast.stmts.get(l).loc();
            let mut d = self.diag(DiagId::sema_implicit_fallthrough, at);
            if into.annotated == 0 && !at.is_macro() {
                if !self.ift_break_only(l, &parents, &kids) {
                    let spelling = self.fallthrough_spelling(at);
                    let n = self
                        .diag(DiagId::note_insert_fallthrough_fixit, at)
                        .arg(spelling.as_str())
                        .fixit(FixIt::insert(at, format!("{spelling}; ")));
                    d = d.with_note(n);
                }
                let n = self
                    .diag(DiagId::note_insert_break_fixit, at)
                    .fixit(FixIt::insert(at, "break; "));
                d = d.with_note(n);
            }
            self.emit(d);
        }
    }

    /// Collects, in source order, the case and default labels of `s`, the
    /// position of each compound statement's children, the children
    /// themselves and the switch bodies.
    fn ift_collect(
        &self,
        s: StmtId,
        labels: &mut Vec<StmtId>,
        parents: &mut Parents,
        kids: &mut Kids,
        switch_bodies: &mut Vec<StmtId>,
    ) {
        let kind = self.ast.stmts.get(s).kind();
        match kind {
            StmtKind::CaseStmt | StmtKind::DefaultStmt => labels.push(s),
            StmtKind::SwitchStmt => {
                switch_bodies.push(self.ast.stmts.switch_data(s).body);
            }
            // Not part of the function's CFG.
            StmtKind::DeclStmt | StmtKind::ExprStmt => return,
            _ => {}
        }
        let ks = self.ift_kids(s);
        for (i, &c) in ks.iter().enumerate() {
            parents.insert(c, (s, i));
            self.ift_collect(c, labels, parents, kids, switch_bodies);
        }
        if kind == StmtKind::CompoundStmt {
            kids.insert(s, ks);
        }
    }

    /// The sub-statements of `s` that can hold a label.
    fn ift_kids(&self, s: StmtId) -> Vec<StmtId> {
        let st = &self.ast.stmts;
        match st.get(s).kind() {
            StmtKind::CaseStmt => vec![st.case_data(s).sub],
            StmtKind::DefaultStmt => vec![st.default_sub(s)],
            StmtKind::LabelStmt | StmtKind::AttributedStmt => vec![st.sub_stmt(s)],
            StmtKind::SwitchStmt => vec![st.switch_data(s).body],
            _ => self.children(s),
        }
    }

    /// The predecessors falling into label `l`.
    fn ift_label_preds(
        &self,
        l: StmtId,
        parents: &Parents,
        kids: &Kids,
        switch_bodies: &[StmtId],
        flow: &Flow,
        into: &mut Preds,
    ) {
        let mut cur = l;
        loop {
            let Some(&(p, i)) = parents.get(&cur) else { return };
            match self.ast.stmts.get(p).kind() {
                StmtKind::CompoundStmt => {
                    let ks = kids.get(&p).map_or(&[][..], Vec::as_slice);
                    for j in (0..i.min(ks.len())).rev() {
                        if self.ift_end(ks[j], flow, into, &ks[..j]) == End::Done {
                            return;
                        }
                    }
                    if switch_bodies.contains(&p) {
                        // The switch itself: good.
                        return;
                    }
                    cur = p;
                }
                // `case 1: case 2:` and `L: case 2:`: good; a
                // `[[likely]]` on the label is transparent.
                StmtKind::CaseStmt | StmtKind::DefaultStmt | StmtKind::LabelStmt if cur == l => {
                    return;
                }
                // `[[likely]] case 1:` and `L: case 1:` (not the label's own
                // sub-statement): transparent.
                StmtKind::AttributedStmt | StmtKind::LabelStmt => cur = p,
                // `case 1: { case 2: }`: the first label's block is empty
                // and its sub-statement is clang's last statement.
                StmtKind::CaseStmt | StmtKind::DefaultStmt => {
                    into.unannotated += 1;
                    return;
                }
                // `if (c) { case 1: }`, `while (c) { case 1: }`: the
                // block entered from the condition (clang's condition
                // block, an unannotated predecessor when reached).
                StmtKind::IfStmt | StmtKind::WhileStmt | StmtKind::ForStmt
                    if self.ift_branch_of(p, cur) =>
                {
                    if flow.before(cur) {
                        into.unannotated += 1;
                    }
                    return;
                }
                // `do { case 1: } while (c);`: the label starts the body,
                // entered from before the `do` and from the condition
                // (clang's loop-back block, empty, so its predecessor,
                // the condition block ending in the `do`).
                StmtKind::DoStmt if self.ast.stmts.loop_data(p).body == Some(cur) => {
                    if flow.do_back.get(&p).copied().unwrap_or(false) {
                        into.unannotated += 1;
                    }
                    cur = p;
                }
                // A range-`for` body (jumping past its declarations is
                // ill-formed), a `try` block or a handler: not followed.
                _ => return,
            }
        }
    }

    /// Whether the condition of `do` statement `d` is reached (the body
    /// falls off its end or a `continue` of this loop is reached) and is
    /// not a constant false (clang prunes that edge).
    fn ift_do_back(&mut self, d: StmtId, flow: &Flow) -> bool {
        let l = *self.ast.stmts.loop_data(d);
        let Some(body) = l.body else { return false };
        let reached = flow.after(body) || self.ift_has_continue(body, flow);
        reached && l.cond.is_none_or(|c| self.known_truth(c) != Some(false))
    }

    /// `s` holds a reached `continue` of the enclosing loop.
    fn ift_has_continue(&self, s: StmtId, flow: &Flow) -> bool {
        match self.ast.stmts.get(s).kind() {
            StmtKind::ContinueStmt => flow.before(s),
            StmtKind::WhileStmt
            | StmtKind::ForStmt
            | StmtKind::DoStmt
            | StmtKind::CXXForRangeStmt
            | StmtKind::CXXExpansionStmt
            | StmtKind::DeclStmt
            | StmtKind::ExprStmt => false,
            _ => self.children(s).into_iter().any(|c| self.ift_has_continue(c, flow)),
        }
    }

    /// Whether `s` is a branch of `if` statement `p`, or the body of
    /// `while` or `for` statement `p`.
    fn ift_branch_of(&self, p: StmtId, s: StmtId) -> bool {
        let st = &self.ast.stmts;
        if st.get(p).kind() == StmtKind::IfStmt {
            let d = st.if_data(p);
            d.then == Some(s) || d.els == Some(s)
        } else {
            st.loop_data(p).body == Some(s)
        }
    }

    /// Adds the predecessor ending with statement `s` to `into`; `End::
    /// Empty` when `s` holds no CFG element. `before` are the statements
    /// before `s` in its block (an unreachable predecessor is searched
    /// backwards for an annotation).
    fn ift_end(&self, s: StmtId, flow: &Flow, into: &mut Preds, before: &[StmtId]) -> End {
        let st = &self.ast.stmts;
        match st.get(s).kind() {
            StmtKind::NullStmt => End::Empty,
            StmtKind::CompoundStmt => {
                let inner = self.children(s);
                for j in (0..inner.len()).rev() {
                    if self.ift_end(inner[j], flow, into, &inner[..j]) == End::Done {
                        return End::Done;
                    }
                }
                End::Empty
            }
            StmtKind::CaseStmt | StmtKind::DefaultStmt => {
                let sub = self.ift_kids(s)[0];
                if self.ift_end(sub, flow, into, &[]) == End::Empty {
                    // clang `getLastStmt`: an empty case block ends with its
                    // sub-statement (`case 1: ;`, `case 1: {}`).
                    into.unannotated += 1;
                }
                End::Done
            }
            StmtKind::AttributedStmt if self.ift_is_fallthrough(s) => {
                into.annotated += 1;
                if !flow.before(s) {
                    into.dead.push(s);
                }
                End::Done
            }
            StmtKind::LabelStmt | StmtKind::AttributedStmt => {
                self.ift_end(st.sub_stmt(s), flow, into, &[])
            }
            _ if !flow.before(s) => {
                // An unreachable straight-line run falling into the label:
                // its last annotation, if any.
                if matches!(st.get(s).kind(), StmtKind::ExprStmt | StmtKind::DeclStmt) {
                    for &k in before.iter().rev() {
                        match st.get(k).kind() {
                            StmtKind::AttributedStmt if self.ift_is_fallthrough(k) => {
                                into.annotated += 1;
                                into.dead.push(k);
                                break;
                            }
                            StmtKind::ExprStmt | StmtKind::DeclStmt | StmtKind::NullStmt => {}
                            _ => break,
                        }
                    }
                }
                End::Done
            }
            StmtKind::IfStmt => {
                let d = *st.if_data(s);
                for b in [d.then, d.els].into_iter().flatten() {
                    // An empty branch ends with the condition.
                    if flow.after(b) && self.ift_end(b, flow, into, &[]) == End::Empty {
                        into.unannotated += 1;
                    }
                }
                // The false edge of an `if` with no else.
                if d.els.is_none() && flow.after(s) {
                    into.unannotated += 1;
                }
                End::Done
            }
            _ => {
                if flow.after(s) {
                    into.unannotated += 1;
                }
                End::Done
            }
        }
    }

    /// clang: the block of label `l`, past empty case blocks, holds
    /// nothing but a `break` (only the `break;` note).
    fn ift_break_only(&self, l: StmtId, parents: &Parents, kids: &Kids) -> bool {
        let st = &self.ast.stmts;
        let mut s = l;
        loop {
            if let Some(f) = self.ift_first(s) {
                return st.get(f).kind() == StmtKind::BreakStmt;
            }
            // The block goes on with the next statement.
            let mut cur = s;
            loop {
                let Some(&(p, i)) = parents.get(&cur) else { return false };
                match st.get(p).kind() {
                    StmtKind::CompoundStmt => {
                        if let Some(&n) = kids.get(&p).and_then(|k| k.get(i + 1)) {
                            s = n;
                            break;
                        }
                        cur = p;
                    }
                    StmtKind::CaseStmt
                    | StmtKind::DefaultStmt
                    | StmtKind::LabelStmt
                    | StmtKind::AttributedStmt => cur = p,
                    _ => return false,
                }
            }
        }
    }

    /// The first CFG element (or terminator) of `s`, past labels; `None`
    /// when it has none.
    fn ift_first(&self, s: StmtId) -> Option<StmtId> {
        let st = &self.ast.stmts;
        match st.get(s).kind() {
            StmtKind::NullStmt => None,
            StmtKind::CompoundStmt => self.children(s).into_iter().find_map(|c| self.ift_first(c)),
            StmtKind::CaseStmt | StmtKind::DefaultStmt | StmtKind::LabelStmt => {
                self.ift_first(self.ift_kids(s)[0])
            }
            StmtKind::AttributedStmt if !self.ift_is_fallthrough(s) => {
                self.ift_first(st.sub_stmt(s))
            }
            _ => Some(s),
        }
    }

    /// `s` is an attributed statement with `[[fallthrough]]`.
    pub(crate) fn ift_is_fallthrough(&self, s: StmtId) -> bool {
        self.ast
            .lists
            .iter::<kiln_ast::AttrId>(self.ast.stmts.stmt_attrs(s))
            .any(|a| self.ast.attrs.get(a).kind == AttrKind::FALLTHROUGH)
    }
}

impl Sema<'_, '_> {
    /// Copies the preprocessor's attribute-macro history (see
    /// [`AttrMacro`]) if its generation `generation` differs from the copy's.
    /// The parser calls this before finishing a function or lambda body.
    pub fn sync_attr_macros(&mut self, generation: u32, macros: impl FnOnce() -> Vec<AttrMacro>) {
        if self.attr_macros.0 != generation {
            self.attr_macros = (generation, macros());
        }
    }

    /// The spelling the fall-through note suggests at `at` (clang
    /// `getFallthroughAttrSpelling`): the name of the latest object-like
    /// macro in effect at `at` spelled `[[clang::fallthrough]]` (preferred
    /// before C++17) or `[[fallthrough]]`, else the attribute itself.
    fn fallthrough_spelling(&self, at: Loc) -> String {
        let prefer_clang = self.std < Std::Cxx17;
        let order: &[&str] =
            if prefer_clang { &[CLANG_TOKENS, STD_TOKENS] } else { &[STD_TOKENS, CLANG_TOKENS] };
        for tokens in order {
            if let Some(name) = self.last_macro_with_spelling(at, tokens) {
                return name.to_owned();
            }
        }
        if prefer_clang { "[[clang::fallthrough]]" } else { "[[fallthrough]]" }.to_owned()
    }

    /// clang `Preprocessor::getLastMacroWithSpelling`: among the macros
    /// spelled `tokens` whose definition is in effect at `at`, the one
    /// defined last.
    fn last_macro_with_spelling(&self, at: Loc, tokens: &str) -> Option<&str> {
        let sm = self.diags.sm();
        let mut best: Option<&AttrMacro> = None;
        for m in &self.attr_macros.1 {
            if *m.tokens != *tokens
                || !before_in_tu(sm, m.def, at)
                || (m.end.is_valid() && !before_in_tu(sm, at, m.end))
            {
                continue;
            }
            if best.is_none_or(|b| before_in_tu(sm, b.def, m.def)) {
                best = Some(m);
            }
        }
        best.map(|m| &*m.name)
    }
}

/// True if `a` precedes `b` in the translation unit (clang
/// `isBeforeInTranslationUnit`), comparing the two locations' include
/// chains at their nearest common file. A location in a buffer outside
/// the main file's include tree (the predefines, the command line)
/// precedes everything.
pub(crate) fn before_in_tu(sm: &SourceManager, a: Loc, b: Loc) -> bool {
    let chain = |l: Loc| {
        let mut v = Vec::new();
        let mut l = sm.expansion_loc(l);
        while let Some((f, off)) = sm.offset_of(l) {
            v.push((f, off));
            let inc = sm.include_loc(l);
            if !inc.is_valid() || v.len() > 256 {
                break;
            }
            l = sm.expansion_loc(inc);
        }
        v
    };
    let (ca, cb) = (chain(a), chain(b));
    for (i, &(fa, oa)) in ca.iter().enumerate() {
        if let Some(j) = cb.iter().position(|&(fb, _)| fb == fa) {
            let ob = cb[j].1;
            // At the same `#include`, the included file's text comes
            // after the directive itself.
            return oa < ob || (oa == ob && i < j);
        }
    }
    true
}
