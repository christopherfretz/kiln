//! Placement of `[[fallthrough]]` ([dcl.attr.fallthrough]/1; clang
//! `DiagnoseSwitchLabelsFallthrough`, spec/sema.md "Statement
//! attributes"): a fallthrough statement must directly precede a switch
//! label, judged on the body's control flow once the body is complete.

use std::collections::{HashMap, HashSet};

use kiln_ast::attr::AttrKind;
use kiln_ast::stmt::StmtKind;
use kiln_ast::{DeclId, StmtId};
use kiln_diag::DiagId;

use crate::Sema;

/// What control reaches after a statement: the case or default label
/// it falls into, if any.
type Next = Option<StmtId>;

/// The fallthrough statements of a body and the label each falls into.
#[derive(Default)]
struct Walk {
    /// Every fallthrough statement with the label it
    /// directly precedes (`next`) and the label ending its straight-line
    /// run (`skip`; the CFG block that holds it).
    found: Vec<(StmtId, Next, Next)>,
}

impl Sema<'_, '_> {
    /// The CFG-based checks clang runs after the return-type check
    /// (`AnalysisBasedWarnings::IssueWarnings`): the fallthrough placement
    /// when the body built a fallthrough statement (an instantiation
    /// copies them, so is not checked), then the infinite recursion
    /// (`expr_start`: [`Sema::check_infinite_recursion`]), then a throw in
    /// a non-throwing function.
    pub(crate) fn late_analysis(
        &mut self,
        f: DeclId,
        body: StmtId,
        has_fallthrough: bool,
        expr_start: u32,
        uninit: bool,
    ) {
        // clang `DiagnoseSwitchLabelsFallthrough`: the unannotated
        // fall-throughs, then the misplaced annotations; the unreachable
        // code (clang `CheckUnreachable`) comes first.
        self.check_unreachable_code(body);
        if uninit {
            self.check_uninitialized(f, body);
        }
        self.check_implicit_fallthrough(body);
        if has_fallthrough {
            self.check_fallthrough_placement(body);
        }
        self.check_infinite_recursion(f, body, expr_start);
        self.check_throw_in_nothrow(f, body);
    }

    /// `fallthrough annotation does not directly precede switch label`
    /// at each misplaced fallthrough statement. A reachable one must be
    /// followed, past null statements and empty blocks, by a case or
    /// default label (of any switch: the end of an inner switch falls
    /// into the next label of the outer one); an unreachable one only
    /// needs to be the last fallthrough of a straight-line run that falls
    /// into a label (clang scans an unreachable predecessor block
    /// backwards). Every case label counts as reachable. With no CFG
    /// (`if consteval`) nothing is checked.
    fn check_fallthrough_placement(&mut self, body: StmtId) {
        let mut w = Walk::default();
        self.ft_walk(body, None, None, &mut w);
        if w.found.is_empty() {
            return;
        }
        let wanted: Vec<StmtId> = w.found.iter().map(|f| f.0).collect();
        let Some((before, _)) = self.with_cfg(body, |_, c| c.stmt_flow()) else { return };
        let dead: HashSet<StmtId> =
            wanted.into_iter().filter(|s| !before.get(s).copied().unwrap_or(false)).collect();
        // clang erases each valid annotation from a `SmallPtrSet<_, 8>`
        // as it visits the labels in order (the last element moves into
        // the hole), then reports what is left in set order; past 8
        // elements the order is the pointers', here source order.
        let mut order = HashMap::new();
        self.ft_preorder(body, &mut order);
        let rank = |s: StmtId| order.get(&s).copied().unwrap_or(usize::MAX);
        w.found.sort_by_key(|f| rank(f.0));
        let mut valid: Vec<(usize, usize, StmtId)> = w
            .found
            .iter()
            .enumerate()
            .filter_map(|(i, &(s, next, skip))| {
                let label = if dead.contains(&s) { skip } else { next };
                label.map(|l| (rank(l), i, s))
            })
            .collect();
        valid.sort_unstable();
        let mut left: Vec<StmtId> = w.found.iter().map(|f| f.0).collect();
        let small = left.len() <= 8;
        for (_, _, s) in valid {
            if let Some(i) = left.iter().position(|&x| x == s) {
                if small {
                    left.swap_remove(i);
                } else {
                    left.remove(i);
                }
            }
        }
        for s in left {
            let loc = self.ast.stmts.get(s).loc();
            self.error(DiagId::sema_fallthrough_invalid_placement, loc);
        }
    }

    /// Records the fallthrough statements of `s`, given what follows it
    /// (`next`) and what ends its straight-line run (`skip`).
    fn ft_walk(&self, s: StmtId, next: Next, skip: Next, w: &mut Walk) {
        let st = &self.ast.stmts;
        match st.get(s).kind() {
            StmtKind::CompoundStmt => {
                let (mut next, mut skip) = (next, skip);
                for c in self.children(s).into_iter().rev() {
                    self.ft_walk(c, next, skip, w);
                    (next, skip) = self.ft_before(c, next, skip);
                }
            }
            StmtKind::AttributedStmt if self.is_fallthrough_stmt(s) => {
                w.found.push((s, next, skip));
            }
            StmtKind::AttributedStmt
            | StmtKind::LabelStmt
            | StmtKind::CaseStmt
            | StmtKind::DefaultStmt => {
                for c in self.children(s) {
                    self.ft_walk(c, next, skip, w);
                }
            }
            // A new block starts after a branch, a switch or a try.
            StmtKind::IfStmt | StmtKind::SwitchStmt | StmtKind::CXXTryStmt => {
                for c in self.children(s) {
                    self.ft_walk(c, next, next, w);
                }
            }
            StmtKind::CXXCatchStmt => self.ft_walk(st.catch_block(s), next, next, w),
            // A loop body flows back to the condition.
            StmtKind::WhileStmt
            | StmtKind::ForStmt
            | StmtKind::DoStmt
            | StmtKind::CXXForRangeStmt
            | StmtKind::CXXExpansionStmt => {
                for c in self.children(s) {
                    self.ft_walk(c, None, None, w);
                }
            }
            _ => {}
        }
    }

    /// What follows the statement before `c`, given what follows `c`.
    fn ft_before(&self, c: StmtId, next: Next, skip: Next) -> (Next, Next) {
        match self.ast.stmts.get(c).kind() {
            StmtKind::CaseStmt | StmtKind::DefaultStmt => (Some(c), Some(c)),
            // `do { case 1: } while (c);`: the label starts the body's
            // block, which the statement before the `do` falls into.
            StmtKind::DoStmt if let Some(l) = self.ft_do_entry_label(c) => (Some(l), Some(l)),
            // `[[likely]] case 1:`: only a fallthrough statement is a CFG
            // element of its own (clang `CFGBuilder::VisitAttributedStmt`).
            StmtKind::AttributedStmt if !self.is_fallthrough_stmt(c) => {
                self.ft_before(self.ast.stmts.sub_stmt(c), next, skip)
            }
            _ if self.ft_transparent(c) => (next, skip),
            StmtKind::ExprStmt | StmtKind::DeclStmt => (None, skip),
            _ => (None, None),
        }
    }

    /// The case or default label that starts the body of `do` statement
    /// `d`, past null statements, empty blocks and non-fallthrough
    /// attributes.
    fn ft_do_entry_label(&self, d: StmtId) -> Option<StmtId> {
        let st = &self.ast.stmts;
        let mut s = st.loop_data(d).body?;
        loop {
            match st.get(s).kind() {
                StmtKind::CaseStmt | StmtKind::DefaultStmt => return Some(s),
                StmtKind::CompoundStmt => {
                    s = self.children(s).into_iter().find(|&c| !self.ft_transparent(c))?;
                }
                StmtKind::AttributedStmt if !self.is_fallthrough_stmt(s) => s = st.sub_stmt(s),
                _ => return None,
            }
        }
    }

    /// `s` adds nothing to the CFG: a null statement or a block of such.
    fn ft_transparent(&self, s: StmtId) -> bool {
        match self.ast.stmts.get(s).kind() {
            StmtKind::NullStmt => true,
            StmtKind::CompoundStmt => self.children(s).into_iter().all(|c| self.ft_transparent(c)),
            _ => false,
        }
    }

    /// `s` is an attributed statement with `[[fallthrough]]`.
    fn is_fallthrough_stmt(&self, s: StmtId) -> bool {
        self.ast
            .lists
            .iter::<kiln_ast::AttrId>(self.ast.stmts.stmt_attrs(s))
            .any(|a| self.ast.attrs.get(a).kind == AttrKind::FALLTHROUGH)
    }

    /// The source-order index of each statement of `s`.
    fn ft_preorder(&self, s: StmtId, order: &mut HashMap<StmtId, usize>) {
        let n = order.len();
        order.insert(s, n);
        for c in self.children(s) {
            self.ft_preorder(c, order);
        }
    }
}
