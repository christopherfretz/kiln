//! The CFG builder: one forward walk over the sema'd body (clang
//! `CFGBuilder`, which walks backwards; the graph is the same up to
//! block numbering, docs/design/reachability.md §3).
//!
//! The walk keeps a *position*: the open block (`cur`), or the edges
//! still waiting for a target (`dangling`), none when the point is
//! unreachable by construction. A block is created lazily, when an
//! element is pushed, a label or case starts one, or a construct needs a
//! join; every dangling edge then targets it.

use std::collections::{HashMap, HashSet};

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{BinaryOp, ExprKind};
use kiln_ast::stmt::{IfKind, StmtKind};
use kiln_ast::{DeclId, ExprId, StmtId};

use super::{Adj, Block, BlockId, Cfg, Edge, Elem, Pos, Status, Term, Throw, TryInfo, edge};
use crate::Sema;

/// What the consumers of one body's graph need besides the graph.
#[derive(Default, Debug)]
pub(crate) struct CfgReq {
    /// The expressions whose reachability is asked for (the deferred
    /// runtime-behavior diagnostics' anchors).
    pub(crate) probes: HashSet<ExprId>,
    /// Record each statement's entry and exit points
    /// (`-Wimplicit-fallthrough`, misplaced `[[fallthrough]]`).
    pub(crate) flow: bool,
    /// Record the unreachable-code scan's tables (`-Wunreachable-code*`).
    pub(crate) scan: bool,
    /// Record the commas and the multi-variable declarations' per-variable
    /// points (`-Wuninitialized`, [`Cfg::commas`], [`Cfg::decl_marks`]).
    pub(crate) uninit: bool,
}

/// A loop, switch or expansion statement `break`/`continue` leave.
struct Jump {
    is_loop: bool,
    brk: Vec<u32>,
    cont: Vec<u32>,
    /// The automatic variables in scope at the statement.
    dtor_mark: usize,
}

struct SwitchFrame {
    /// The block ending in the switch's condition.
    src: BlockId,
    value: Option<i128>,
    /// The `default:` block.
    default_block: Option<BlockId>,
}

#[derive(Default)]
struct LabelSlot {
    block: Option<BlockId>,
    pending: Vec<u32>,
}

/// The state of one build.
pub(crate) struct Builder {
    pub(crate) cfg: Cfg,
    req: CfgReq,
    max_elems: usize,
    cur: Option<BlockId>,
    dangling: Vec<u32>,
    jumps: Vec<Jump>,
    switches: Vec<SwitchFrame>,
    /// The enclosing try statements (indices into `cfg.tries`), with the
    /// edges into their dispatch blocks, innermost last.
    try_stack: Vec<(u32, Vec<u32>)>,
    labels: HashMap<DeclId, LabelSlot>,
    /// The automatic variables with a non-trivial destructor in scope,
    /// in declaration order.
    dtors: Vec<DeclId>,
    /// The temporaries of the enclosing full-expressions in construction
    /// order, with the conditional operands they sit in; destroyed at
    /// its end.
    temps: Vec<TempTok>,
    /// Enclosing `ExprWithCleanups`; lifetime-extending materializations
    /// since the innermost one.
    cleanups: u32,
    extended: u32,
    cur_inc: Option<StmtId>,
    cur_ret: Option<StmtId>,
    /// The edges of `goto *p` into the shared indirect-goto block.
    igoto: Vec<u32>,
    kids: Vec<ExprId>,
}

/// One entry of a full-expression's temporaries record (clang
/// `TempDtorContext`).
#[derive(Clone, Copy)]
enum TempTok {
    Temp(ExprId),
    /// A conditional operand starts: whether it is known to be
    /// evaluated.
    Open(Option<bool>),
    Close,
    /// The next two operands are the arms of one `?:`.
    Arms,
}

/// clang `bothKnownTrue`.
fn both_known(a: Option<bool>, b: Option<bool>) -> Option<bool> {
    Some(a? && b?)
}

/// The length of the operand starting at `toks[0]` (an `Open`).
fn group_len(toks: &[TempTok]) -> usize {
    let mut depth = 0u32;
    for (i, t) in toks.iter().enumerate() {
        match t {
            TempTok::Open(_) => depth += 1,
            TempTok::Close => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
    }
    toks.len()
}

/// The first temporary of an operand list outside its nested operands
/// (clang's decision point).
fn first_temp(toks: &[TempTok]) -> Option<ExprId> {
    let mut depth = 0u32;
    for t in toks {
        match *t {
            TempTok::Open(_) => depth += 1,
            TempTok::Close => depth -= 1,
            TempTok::Temp(e) if depth == 0 => return Some(e),
            _ => {}
        }
    }
    None
}

/// The scan's reading of a branch condition: an `if constexpr` and a
/// configuration value prune nothing.
#[derive(Clone, Copy)]
struct Truth {
    base: Option<bool>,
    scan: Option<bool>,
}

impl Truth {
    const UNKNOWN: Truth = Truth { base: None, scan: None };

    /// The flags of the edge taken when the condition is `v`.
    fn flags(self, v: bool, null: bool) -> u8 {
        let mut f = 0;
        if self.base != Some(!v) {
            f |= edge::LIVE;
        } else if null {
            f |= edge::NULL;
        }
        if self.scan != Some(!v) {
            f |= edge::SCAN;
        }
        f
    }
}

/// A branch condition's outgoing edges (clang `VisitLogicalOperator`
/// sends a `&&`/`||` condition's short-circuit edges straight to the
/// statement's targets).
struct CondOut {
    t: Vec<u32>,
    f: Vec<u32>,
}

impl Builder {
    pub(crate) fn new(cfg: Cfg, req: CfgReq, max_elems: usize) -> Builder {
        Builder {
            cfg,
            req,
            max_elems,
            cur: None,
            dangling: Vec::new(),
            jumps: Vec::new(),
            switches: Vec::new(),
            try_stack: Vec::new(),
            labels: HashMap::new(),
            dtors: Vec::new(),
            temps: Vec::new(),
            cleanups: 0,
            extended: 0,
            cur_inc: None,
            cur_ret: None,
            igoto: Vec::new(),
            kids: Vec::new(),
        }
    }

    fn ok(&self) -> bool {
        self.cfg.status == Status::Ok
    }

    fn new_block(&mut self) -> BlockId {
        let n = self.cfg.elems.len() as u32;
        let id = BlockId(self.cfg.blocks.len() as u32);
        self.cfg.blocks.push(Block { elems: n..n, ..Block::default() });
        id
    }

    /// An edge out of `from`, its target not known yet.
    fn edge(&mut self, from: BlockId, flags: u8) -> u32 {
        let i = self.cfg.edges.len() as u32;
        self.cfg.edges.push(Edge { from, to: BlockId::PENDING, flags });
        i
    }

    fn resolve(&mut self, edges: &[u32], to: BlockId) {
        for &e in edges {
            self.cfg.edges[e as usize].to = to;
        }
    }

    /// Ends the open block with a fall-through edge.
    fn close(&mut self) {
        if let Some(b) = self.cur.take() {
            let e = self.edge(b, edge::BOTH);
            self.dangling.push(e);
        }
    }

    /// The position as a list of edges (the position becomes nothing).
    fn take_pos(&mut self) -> Vec<u32> {
        self.close();
        std::mem::take(&mut self.dangling)
    }

    /// Continues at the edges `p`.
    fn set_pos(&mut self, p: Vec<u32>) {
        debug_assert!(self.cur.is_none());
        self.dangling = p;
    }

    /// Adds the edges `p` to the position.
    fn join(&mut self, p: Vec<u32>) {
        self.close();
        self.dangling.extend(p);
    }

    /// The point is reachable by construction (an open block or a
    /// dangling edge).
    fn has_pos(&self) -> bool {
        self.cur.is_some() || !self.dangling.is_empty()
    }

    /// The open block, created on the dangling edges when there is none.
    fn open(&mut self) -> BlockId {
        if let Some(b) = self.cur {
            return b;
        }
        let b = self.new_block();
        let d = std::mem::take(&mut self.dangling);
        self.resolve(&d, b);
        self.dangling = d;
        self.dangling.clear();
        self.cur = Some(b);
        b
    }

    /// A new block on the current position.
    fn fresh(&mut self) -> BlockId {
        self.close();
        self.open()
    }

    /// Ends the open block (created if needed) with terminator `t`.
    fn terminate(&mut self, t: Term) -> BlockId {
        let b = self.open();
        self.cfg.blocks[b.idx()].term = t;
        self.cur = None;
        b
    }

    /// Ends the open block with one edge into the list `to`.
    fn jump(&mut self, t: Term, flags: u8) -> u32 {
        let b = self.terminate(t);
        self.edge(b, flags)
    }

    /// Ends the open block in the exit (a return, a throw leaving the
    /// body, a `noreturn` call).
    fn jump_to_exit(&mut self, flags: u8) {
        let b = self.open();
        self.cur = None;
        let e = self.edge(b, flags);
        self.cfg.edges[e as usize].to = BlockId::EXIT;
    }

    fn pos(&mut self) -> Pos {
        if let Some(b) = self.cur {
            Pos::Block(b)
        } else {
            let s = self.cfg.pos_edges.len() as u32;
            self.cfg.pos_edges.extend_from_slice(&self.dangling);
            Pos::Edges(s, self.dangling.len() as u32)
        }
    }

    fn push(&mut self, el: Elem) {
        if self.cfg.elems.len() >= self.max_elems {
            self.cfg.status = Status::TooLarge;
            return;
        }
        let b = self.open();
        let i = self.cfg.elems.len() as u32;
        debug_assert_eq!(self.cfg.blocks[b.idx()].elems.end, i, "block elements are contiguous");
        self.cfg.elems.push(el);
        self.cfg.blocks[b.idx()].elems.end = i + 1;
        if let Some(u) = el.unit()
            && self.req.scan
        {
            if let Some(l) = self.cur_inc {
                self.cfg.inc.insert(u, l);
            }
            if let Some(r) = self.cur_ret {
                self.cfg.ret.insert(u, r);
            }
        }
    }

    /// Records a full-expression temporaries entry (inside one).
    fn temp_tok(&mut self, t: TempTok) {
        if self.cleanups > 0 {
            self.temps.push(t);
        }
    }

    /// The flow table's point: the open block, or its dangling edges.
    fn flow_pos(&mut self) -> Option<Pos> {
        self.req.flow.then(|| self.pos())
    }
}

impl Sema<'_, '_> {
    /// Builds the graph of function `f`'s body `body` into `cfg` (cleared
    /// first), with the side tables `req` asks for.
    pub(crate) fn build_cfg(
        &mut self,
        f: Option<DeclId>,
        body: StmtId,
        cfg: Cfg,
        req: CfgReq,
        max_elems: usize,
    ) -> Cfg {
        let mut b = Builder::new(cfg, req, max_elems);
        b.cfg.clear();
        b.cfg.body = Some(body);
        b.cfg.func = f;
        let entry = b.new_block();
        let exit = b.new_block();
        debug_assert_eq!((entry, exit), (BlockId::ENTRY, BlockId::EXIT));
        let e = b.edge(BlockId::ENTRY, edge::BOTH);
        b.dangling.push(e);
        if let Some(f) = f
            && self.kind_of(f) == DeclKind::CXXConstructor
        {
            let inits: Vec<ExprId> = self.ast.ctor_inits(f).map(|i| i.init).collect();
            for (i, init) in inits.into_iter().enumerate() {
                self.cfg_expr(init, &mut b);
                b.push(Elem::Init(i as u32));
            }
        }
        self.cfg_stmt(body, &mut b);
        b.close();
        let d = std::mem::take(&mut b.dangling);
        b.resolve(&d, BlockId::EXIT);
        if !b.igoto.is_empty() {
            let g = b.new_block();
            b.cfg.blocks[g.idx()].term = Term::IndirectGoto;
            let ig = std::mem::take(&mut b.igoto);
            b.resolve(&ig, g);
            for l in self.addr_taken_labels(body) {
                if let Some(t) = b.labels.get(&l).and_then(|s| s.block) {
                    let e = b.edge(g, edge::BOTH);
                    b.cfg.edges[e as usize].to = t;
                }
            }
        }
        let mut cfg = b.cfg;
        finalize(&mut cfg);
        cfg
    }

    fn cfg_stmt(&mut self, s: StmtId, b: &mut Builder) {
        if !b.ok() {
            return;
        }
        let entry = b.flow_pos();
        self.cfg_stmt_inner(s, b);
        if let Some(entry) = entry {
            let exit = b.pos();
            b.cfg.flow.push((s, entry, exit));
        }
    }

    #[allow(clippy::too_many_lines, reason = "one arm per statement kind")]
    fn cfg_stmt_inner(&mut self, s: StmtId, b: &mut Builder) {
        let st = &self.ast.stmts;
        match st.get(s).kind() {
            StmtKind::CompoundStmt => {
                let mark = b.dtors.len();
                let list = st.compound_body(s);
                for i in 0..self.ast.lists.len(list) {
                    let c = self.ast.lists.at::<StmtId>(list, i).expect("compound child");
                    self.cfg_stmt(c, b);
                }
                self.cfg_scope_end(mark, b);
            }
            StmtKind::LabelStmt => {
                let l = st.label_decl(s);
                let sub = st.sub_stmt(s);
                let blk = b.fresh();
                b.cfg.blocks[blk.idx()].label = Some(s);
                let slot = b.labels.entry(l).or_default();
                slot.block = Some(blk);
                let p = std::mem::take(&mut slot.pending);
                b.resolve(&p, blk);
                self.cfg_stmt(sub, b);
            }
            StmtKind::AttributedStmt => {
                let sub = st.sub_stmt(s);
                if self.ift_is_fallthrough(s) {
                    b.push(Elem::Stmt(s));
                }
                self.cfg_stmt(sub, b);
            }
            StmtKind::GotoStmt => {
                let l = st.label_decl(s);
                b.push(Elem::Stmt(s));
                let e = b.jump(Term::Jump(s), edge::BOTH);
                let slot = b.labels.entry(l).or_default();
                match slot.block {
                    Some(t) => b.cfg.edges[e as usize].to = t,
                    None => slot.pending.push(e),
                }
            }
            StmtKind::IndirectGotoStmt => {
                self.cfg_expr(st.indirect_goto_target(s), b);
                b.push(Elem::Stmt(s));
                let e = b.jump(Term::Jump(s), edge::BOTH);
                b.igoto.push(e);
            }
            StmtKind::GCCAsmStmt => {
                // clang `VisitChildren`: the outputs, then the inputs, are
                // elements before the statement (or the `asm goto`
                // terminator); the uninitialized-values analysis reads them.
                for i in 0..self.ast.stmts.asm_operands(s).len() {
                    let e = self.ast.stmts.asm_operands(s)[i].expr;
                    self.cfg_expr(e, b);
                }
                if self.ast.stmts.asm_bits(s) & kiln_ast::stmt::asm_bits::GOTO != 0 && b.has_pos() {
                    let labels = self.ast.stmts.asm_data(s).labels;
                    let t = b.terminate(Term::Asm(s));
                    for i in 0..self.ast.lists.len(labels) {
                        let l = self.ast.lists.at::<DeclId>(labels, i).expect("asm label");
                        let e = b.edge(t, edge::BOTH);
                        let slot = b.labels.entry(l).or_default();
                        match slot.block {
                            Some(to) => b.cfg.edges[e as usize].to = to,
                            None => slot.pending.push(e),
                        }
                    }
                    let e = b.edge(t, edge::BOTH);
                    b.dangling.push(e);
                }
            }
            StmtKind::ReturnStmt | StmtKind::CoreturnStmt => {
                let outer = b.cur_ret.replace(s);
                if st.get(s).kind() == StmtKind::ReturnStmt {
                    if let Some(v) = st.return_value(s) {
                        self.cfg_expr(v, b);
                    }
                } else {
                    let (o, p) = (st.coreturn_operand(s), st.coreturn_promise_call(s));
                    for e in o.into_iter().chain(p) {
                        self.cfg_expr(e, b);
                    }
                }
                b.push(Elem::Stmt(s));
                b.cur_ret = outer;
                if self.cfg_dtors(0, b) {
                    b.jump_to_exit(edge::BOTH | edge::RETURN);
                }
            }
            StmtKind::BreakStmt | StmtKind::ContinueStmt => {
                let brk = st.get(s).kind() == StmtKind::BreakStmt;
                b.push(Elem::Stmt(s));
                let frame = if brk {
                    b.jumps.len().checked_sub(1)
                } else {
                    b.jumps.iter().rposition(|j| j.is_loop)
                };
                let Some(fi) = frame else {
                    // clang `badCFG`: a jump in a loop header with no
                    // target in the graph (spec/sema.md §7.4a) leaves the
                    // body without one.
                    b.cfg.status = Status::NoCfg;
                    b.terminate(Term::Jump(s));
                    return;
                };
                if self.cfg_dtors(b.jumps[fi].dtor_mark, b) {
                    let e = b.jump(Term::Jump(s), edge::BOTH);
                    let j = &mut b.jumps[fi];
                    if brk { j.brk.push(e) } else { j.cont.push(e) }
                }
            }
            StmtKind::IfStmt => self.cfg_if(s, b),
            StmtKind::WhileStmt | StmtKind::ForStmt => self.cfg_loop(s, b),
            StmtKind::DoStmt => self.cfg_do(s, b),
            StmtKind::CXXForRangeStmt => self.cfg_range_for(s, b),
            StmtKind::CXXExpansionStmt => self.cfg_expansion(s, b),
            StmtKind::SwitchStmt => self.cfg_switch(s, b),
            StmtKind::CaseStmt => {
                let sub = st.case_data(s).sub;
                let hit = match b.switches.last().map(|f| f.value) {
                    Some(Some(v)) => Self::in_case(self.case_bounds(s), v),
                    _ => true,
                };
                let blk = b.fresh();
                b.cfg.blocks[blk.idx()].label = Some(s);
                b.cfg.case_blocks.push(blk);
                if let Some(src) = b.switches.last().map(|f| f.src) {
                    let flags = if hit { edge::BOTH } else { edge::SCAN };
                    let e = b.edge(src, flags);
                    b.cfg.edges[e as usize].to = blk;
                }
                self.cfg_stmt(sub, b);
            }
            StmtKind::DefaultStmt => {
                let sub = st.default_sub(s);
                let blk = b.fresh();
                b.cfg.blocks[blk.idx()].label = Some(s);
                b.cfg.case_blocks.push(blk);
                if let Some(f) = b.switches.last_mut() {
                    f.default_block = Some(blk);
                }
                self.cfg_stmt(sub, b);
            }
            StmtKind::CXXTryStmt => self.cfg_try(s, b),
            StmtKind::DeclStmt => {
                let list = st.decl_stmt_decls(s);
                let mut has_var = false;
                // clang splits a multi-variable declaration into one
                // statement per variable; the uninitialized-values
                // analysis needs each variable's point.
                let marks = b.req.uninit && self.ast.lists.len(list) > 1;
                for i in 0..self.ast.lists.len(list) {
                    let d = self.ast.lists.at::<DeclId>(list, i).expect("decl");
                    if let Some(v) = self.ast.decls.var(d) {
                        has_var = true;
                        if let Some(init) = v.init {
                            self.cfg_expr(init, b);
                        }
                        if marks {
                            let at = b.cfg.elems.len() as u32;
                            b.cfg.decl_marks.push((at, d));
                        }
                    }
                }
                if has_var {
                    b.push(Elem::Stmt(s));
                    for i in 0..self.ast.lists.len(list) {
                        let d = self.ast.lists.at::<DeclId>(list, i).expect("decl");
                        self.cfg_scope_var(d, b);
                    }
                }
            }
            StmtKind::ExprStmt => self.cfg_expr(st.expr_stmt_expr(s), b),
            StmtKind::CXXCatchStmt => self.cfg_stmt(st.catch_block(s), b),
            // No elements: `;`, a coroutine body's wrapper (built from
            // its user-written body), a contract assertion.
            _ => {}
        }
    }

    /// Notes automatic variable `d` for the destructor calls at the end of
    /// its scope when its type has a non-trivial destructor.
    fn cfg_scope_var(&mut self, d: DeclId, b: &mut Builder) {
        if !self.has_automatic_storage(d) {
            return;
        }
        let t = &self.ast.types;
        let Some(mut q) = self.ast.decls.get(d).ty() else { return };
        while let Some(e) = t.element(q) {
            q = e;
        }
        let Some(c) = t.as_record(q) else { return };
        if self.ast.destructor(c).is_some_and(|x| !self.fn_spec(x).is_trivial()) {
            b.dtors.push(d);
        }
    }

    /// The destructor calls of the automatic variables declared since
    /// `mark` (in reverse), before a jump out of their scopes; `false`
    /// when one is `noreturn` (the block ends in the exit).
    fn cfg_dtors(&mut self, mark: usize, b: &mut Builder) -> bool {
        if !b.has_pos() {
            return true;
        }
        for i in (mark..b.dtors.len()).rev() {
            let d = b.dtors[i];
            b.push(Elem::AutoDtor(d));
            if self.var_dtor_noreturn(d) {
                Self::cfg_noreturn(b);
                return false;
            }
        }
        true
    }

    /// The end of a scope: its variables' destructor calls.
    fn cfg_scope_end(&mut self, mark: usize, b: &mut Builder) {
        if b.dtors.len() > mark {
            self.cfg_dtors(mark, b);
            b.dtors.truncate(mark);
        }
    }

    fn var_dtor_noreturn(&self, d: DeclId) -> bool {
        let t = &self.ast.types;
        let Some(mut q) = self.ast.decls.get(d).ty() else { return false };
        while let Some(e) = t.element(q) {
            q = e;
        }
        t.as_record(q)
            .and_then(|c| self.ast.destructor(c))
            .is_some_and(|x| self.ast.is_noreturn_fn(x))
    }

    /// Ends the open block in a `noreturn` call (clang: a `NORETURN`
    /// block whose only successor is the exit).
    fn cfg_noreturn(b: &mut Builder) {
        let blk = b.open();
        b.cfg.blocks[blk.idx()].noreturn = true;
        b.jump_to_exit(edge::BOTH | edge::ABNORMAL);
    }

    fn cfg_var_init(&mut self, v: Option<DeclId>, b: &mut Builder) {
        if let Some(i) = v.and_then(|v| self.ast.decls.var(v).and_then(|x| x.init)) {
            self.cfg_expr(i, b);
        }
    }

    /// The known truth of branch condition `c` (`whole` for the
    /// configuration-value test).
    fn cfg_truth(&mut self, c: ExprId, whole: ExprId, b: &Builder) -> Truth {
        let base = self.known_truth(c);
        let scan = if base.is_some() && b.req.scan && self.is_configuration_value(whole, &mut None)
        {
            None
        } else {
            base
        };
        Truth { base, scan }
    }

    /// clang `CFGBuilder::VisitLogicalOperator`: the `&&`/`||` `op`, with
    /// no join block between nested operators. Each left operand's
    /// block ends in its operator; the innermost right operand's block
    /// ends in the branch terminator `t` (whole condition `whole`), or,
    /// for an operator used as a value (`t` none), falls through to the
    /// join. `scan_on` is false for an `if constexpr`.
    fn cfg_logical_cond(
        &mut self,
        op: ExprId,
        t: Option<Term>,
        whole: ExprId,
        scan_on: bool,
        b: &mut Builder,
    ) -> CondOut {
        let or = self.ast.exprs.binary_op(op) == BinaryOp::LOr;
        let [l, r] = self.ast.exprs.operands(op);
        let (l, r) = (self.skip_parens(l), self.skip_parens(r));
        let (lt, lf) = if self.is_logical_op(l) {
            let o = self.cfg_logical_cond(l, Some(Term::Logical(op)), op, true, b);
            (o.t, o.f)
        } else {
            self.cfg_expr(l, b);
            let kv = self.cfg_truth(l, op, b);
            let src = b.terminate(Term::Logical(op));
            // clang's order: the true edge first.
            let te = b.edge(src, kv.flags(true, false));
            let fe = b.edge(src, kv.flags(false, false));
            (vec![te], vec![fe])
        };
        let (rhs, short) = if or { (lf, lt) } else { (lt, lf) };
        b.set_pos(rhs);
        // The right operand runs when the left one is true (`&&`) or
        // false (`||`).
        let lv = self.known_truth(l);
        b.temp_tok(TempTok::Open(lv.map(|v| v != or)));
        let mut o = if self.is_logical_op(r) {
            self.cfg_logical_cond(r, t, whole, scan_on, b)
        } else {
            self.cfg_expr(r, b);
            match t {
                None => CondOut { t: b.take_pos(), f: Vec::new() },
                Some(t) => {
                    let base = self.known_truth(r).or_else(|| self.known_truth(op));
                    let scan = if !scan_on
                        || (base.is_some()
                            && b.req.scan
                            && self.is_configuration_value(whole, &mut None))
                    {
                        None
                    } else {
                        base
                    };
                    let kv = Truth { base, scan };
                    let src = b.terminate(t);
                    let te = b.edge(src, kv.flags(true, false));
                    let fe = b.edge(src, kv.flags(false, false));
                    CondOut { t: vec![te], f: vec![fe] }
                }
            }
        };
        b.temp_tok(TempTok::Close);
        let mut v = short;
        if or {
            v.append(&mut o.t);
            o.t = v;
        } else {
            v.append(&mut o.f);
            o.f = v;
        }
        o
    }

    fn cfg_if(&mut self, s: StmtId, b: &mut Builder) {
        // clang 22 builds the CFG of an `if consteval` as a branch on an
        // unknown condition; clang 19 builds none.
        let kind = self.ast.stmts.if_kind(s);
        if self.clang_compat < 22 && matches!(kind, IfKind::Consteval | IfKind::NotConsteval) {
            b.cfg.status = Status::NoCfg;
            return;
        }
        let d = *self.ast.stmts.if_data(s);
        let mark = b.dtors.len();
        if let Some(i) = d.init {
            self.cfg_stmt(i, b);
        }
        self.cfg_var_init(d.cond_var, b);
        if let Some(v) = d.cond_var {
            self.cfg_scope_var(v, b);
        }
        let logical = d.cond.filter(|&c| d.cond_var.is_none() && self.is_logical_op(c));
        let (te, ee) = if let Some(c) = logical {
            let op = self.skip_parens(c);
            let o = self.cfg_logical_cond(op, Some(Term::If(s)), c, kind != IfKind::Constexpr, b);
            (o.t, o.f)
        } else {
            let mut kv = Truth::UNKNOWN;
            if let Some(c) = d.cond {
                self.cfg_expr(c, b);
                // An `if constexpr` condition is a constant expression
                // even naming its condition variable (`if constexpr
                // (constexpr auto o = f())`): an instantiation drops the
                // discarded branch, so its edge must be known dead.
                if d.cond_var.is_none() || kind == IfKind::Constexpr {
                    kv = self.cfg_truth(c, c, b);
                    // The scan prunes no edge of an `if constexpr` (clang
                    // `shouldTreatSuccessorsAsReachable`).
                    if kind == IfKind::Constexpr {
                        kv.scan = None;
                    }
                }
            }
            let src = b.terminate(Term::If(s));
            let te = b.edge(src, kv.flags(true, false));
            let ee = b.edge(src, kv.flags(false, false));
            (vec![te], vec![ee])
        };
        b.set_pos(te);
        if let Some(t) = d.then {
            self.cfg_stmt(t, b);
        }
        let then_out = b.take_pos();
        b.set_pos(ee);
        match d.els {
            Some(e) => {
                self.cfg_stmt(e, b);
                b.join(then_out);
            }
            None => {
                b.join(then_out);
            }
        }
        self.cfg_scope_end(mark, b);
    }

    /// A `while` or `for` loop. clang's graph (not its codegen) targets
    /// a `break` in a `for` loop's header at the loop's exit and a
    /// `continue` there at the enclosing loop; in a `while` condition
    /// both target the enclosing statement (spec/sema.md §7.4a).
    fn cfg_loop(&mut self, s: StmtId, b: &mut Builder) {
        let d = *self.ast.stmts.loop_data(s);
        let mark = b.dtors.len();
        let is_for = self.ast.stmts.get(s).kind() == StmtKind::ForStmt;
        if is_for {
            b.jumps.push(Jump {
                is_loop: false,
                brk: Vec::new(),
                cont: Vec::new(),
                dtor_mark: mark,
            });
        }
        if let Some(i) = d.init {
            self.cfg_stmt(i, b);
        }
        let head = b.fresh();
        self.cfg_var_init(d.cond_var, b);
        let (be, xe) = self.cfg_loop_branch(s, d.cond_var, d.cond, b);
        b.set_pos(be);
        b.jumps.push(Jump { is_loop: true, brk: Vec::new(), cont: Vec::new(), dtor_mark: mark });
        if let Some(body) = d.body {
            self.cfg_stmt(body, b);
        }
        let j = b.jumps.pop().expect("loop frame");
        b.join(j.cont);
        b.fresh();
        if let Some(i) = d.inc {
            let outer = b.cur_inc.replace(s);
            self.cfg_expr(i, b);
            b.cur_inc = outer;
        }
        let back = b.take_pos();
        b.resolve(&back, head);
        b.set_pos(xe);
        b.dangling.extend(j.brk);
        if is_for {
            let h = b.jumps.pop().expect("loop header frame");
            b.dangling.extend(h.brk);
        }
        self.cfg_scope_end(mark, b);
    }

    /// A loop's condition and terminator: the block ending in it, its
    /// truth, and its body and exit edges. A `do` condition is a plain
    /// expression (clang `VisitDoStmt`).
    fn cfg_loop_branch(
        &mut self,
        s: StmtId,
        var: Option<DeclId>,
        cond: Option<ExprId>,
        b: &mut Builder,
    ) -> (Vec<u32>, Vec<u32>) {
        let is_do = self.ast.stmts.get(s).kind() == StmtKind::DoStmt;
        if let Some(c) = cond.filter(|&c| !is_do && var.is_none() && self.is_logical_op(c)) {
            let op = self.skip_parens(c);
            let o = self.cfg_logical_cond(op, Some(Term::Loop(s)), c, true, b);
            return (o.t, o.f);
        }
        let kv = self.cfg_loop_cond(var, cond, b);
        let src = b.terminate(Term::Loop(s));
        let be = b.edge(src, kv.flags(true, true));
        let xe = b.edge(src, kv.flags(false, true));
        (vec![be], vec![xe])
    }

    /// A loop condition (absent: `true`) and its known truth.
    fn cfg_loop_cond(
        &mut self,
        var: Option<DeclId>,
        cond: Option<ExprId>,
        b: &mut Builder,
    ) -> Truth {
        let Some(c) = cond else { return Truth { base: Some(true), scan: Some(true) } };
        self.cfg_expr(c, b);
        if var.is_some() {
            return Truth::UNKNOWN;
        }
        // clang's CFG prunes a loop's constant condition outright, but
        // the scan follows one of `&&` or `||` (a `do`'s; the others go
        // through `cfg_logical_cond`) through its operator edges, where a
        // configuration value does not prune.
        if b.req.scan && self.is_logical_op(c) {
            self.cfg_truth(c, c, b)
        } else {
            let v = self.known_truth(c);
            Truth { base: v, scan: v }
        }
    }

    fn cfg_do(&mut self, s: StmtId, b: &mut Builder) {
        let d = *self.ast.stmts.loop_data(s);
        let mark = b.dtors.len();
        let body_entry = b.fresh();
        b.jumps.push(Jump { is_loop: true, brk: Vec::new(), cont: Vec::new(), dtor_mark: mark });
        if let Some(body) = d.body {
            self.cfg_stmt(body, b);
        }
        let j = b.jumps.pop().expect("loop frame");
        b.join(j.cont);
        b.fresh();
        if let Some(c) = d.cond {
            b.cfg.do_conds.insert(self.ignore_paren_imp_casts(c));
        }
        let (be, xe) = self.cfg_loop_branch(s, None, d.cond, b);
        // clang's back edge goes through an empty block.
        let latch = b.new_block();
        b.resolve(&be, latch);
        let e = b.edge(latch, edge::BOTH);
        b.resolve(&[e], body_entry);
        b.set_pos(xe);
        b.dangling.extend(j.brk);
        self.cfg_scope_end(mark, b);
    }

    /// A range-based `for`: as in clang's graph, a `break` in its
    /// init-statement or range targets the loop's exit (spec/sema.md
    /// §7.4a).
    fn cfg_range_for(&mut self, s: StmtId, b: &mut Builder) {
        let d = *self.ast.stmts.range_for_data(s);
        let mark = b.dtors.len();
        b.jumps.push(Jump { is_loop: false, brk: Vec::new(), cont: Vec::new(), dtor_mark: mark });
        if let Some(i) = d.init {
            self.cfg_stmt(i, b);
        }
        for v in [Some(d.range), d.begin, d.end] {
            self.cfg_var_init(v, b);
        }
        let head = b.fresh();
        if let Some(c) = d.cond {
            self.cfg_expr(c, b);
        }
        let src = b.terminate(Term::Loop(s));
        let be = b.edge(src, edge::BOTH);
        let xe = b.edge(src, edge::BOTH);
        b.set_pos(vec![be]);
        b.jumps.push(Jump { is_loop: true, brk: Vec::new(), cont: Vec::new(), dtor_mark: mark });
        let inner = b.dtors.len();
        self.cfg_var_init(Some(d.loop_var), b);
        self.cfg_scope_var(d.loop_var, b);
        self.cfg_stmt(d.body, b);
        self.cfg_scope_end(inner, b);
        let j = b.jumps.pop().expect("loop frame");
        b.join(j.cont);
        b.fresh();
        if let Some(i) = d.inc {
            let outer = b.cur_inc.replace(s);
            self.cfg_expr(i, b);
            b.cur_inc = outer;
        }
        let back = b.take_pos();
        b.resolve(&back, head);
        b.set_pos(vec![xe]);
        b.dangling.extend(j.brk);
        let h = b.jumps.pop().expect("loop header frame");
        b.dangling.extend(h.brk);
        self.cfg_scope_end(mark, b);
    }

    /// An expansion statement ([stmt.expand]): its init-statement and
    /// prelude, then each element in order; `continue` ends one element
    /// and `break` the statement. A pattern (not expanded) runs its body
    /// zero or more times.
    fn cfg_expansion(&mut self, s: StmtId, b: &mut Builder) {
        let st = &self.ast.stmts;
        let d = *st.expansion_data(s);
        let expanded = st.expansion_expanded(s);
        let mark = b.dtors.len();
        if let Some(i) = d.init {
            self.cfg_stmt(i, b);
        }
        let mut brk = Vec::new();
        if expanded {
            let l = d.prelude;
            for i in 0..self.ast.lists.len(l) {
                let p = self.ast.lists.at::<StmtId>(l, i).expect("prelude");
                self.cfg_stmt(p, b);
            }
            let l = d.expansions;
            for i in 0..self.ast.lists.len(l) {
                let e = self.ast.lists.at::<StmtId>(l, i).expect("expansion");
                b.jumps.push(Jump {
                    is_loop: true,
                    brk: Vec::new(),
                    cont: Vec::new(),
                    dtor_mark: b.dtors.len(),
                });
                self.cfg_stmt(e, b);
                let j = b.jumps.pop().expect("expansion frame");
                brk.extend(j.brk);
                b.join(j.cont);
            }
        } else {
            let t = b.terminate(Term::None);
            let be = b.edge(t, edge::BOTH);
            let skip = b.edge(t, edge::BOTH);
            b.set_pos(vec![be]);
            b.jumps.push(Jump {
                is_loop: true,
                brk: Vec::new(),
                cont: Vec::new(),
                dtor_mark: b.dtors.len(),
            });
            self.cfg_stmt(d.body, b);
            let j = b.jumps.pop().expect("expansion frame");
            brk.extend(j.brk);
            b.take_pos();
            b.set_pos(vec![skip]);
        }
        b.join(brk);
        self.cfg_scope_end(mark, b);
    }

    /// A `switch`: as in clang's graph, a `break` in its header targets
    /// the switch's exit (spec/sema.md §7.4a).
    fn cfg_switch(&mut self, s: StmtId, b: &mut Builder) {
        let d = *self.ast.stmts.switch_data(s);
        let mark = b.dtors.len();
        b.jumps.push(Jump { is_loop: false, brk: Vec::new(), cont: Vec::new(), dtor_mark: mark });
        if let Some(i) = d.init {
            self.cfg_stmt(i, b);
        }
        self.cfg_var_init(d.cond_var, b);
        if let Some(v) = d.cond_var {
            self.cfg_scope_var(v, b);
        }
        self.cfg_expr(d.cond, b);
        let value = if d.cond_var.is_none() { self.fold_int(d.cond) } else { None };
        let cases: Vec<StmtId> = self.ast.lists.iter(d.cases).collect();
        let mut has_default = false;
        let mut matched = false;
        for &c in &cases {
            if self.ast.stmts.get(c).kind() == StmtKind::DefaultStmt {
                has_default = true;
            } else if let Some(v) = value {
                matched |= Self::in_case(self.case_bounds(c), v);
            }
        }
        // clang `isAllEnumCasesCovered`: the default edge of a switch over
        // an enumeration whose enumerators all have cases is unreachable.
        let default_live = !matched && !self.switch_covers_enum(d.cond, &cases, has_default);
        let src = b.terminate(Term::Switch(s));
        b.switches.push(SwitchFrame { src, value, default_block: None });
        self.cfg_stmt(d.body, b);
        let j = b.jumps.pop().expect("switch frame");
        let f = b.switches.pop().expect("switch frame");
        // The default edge (or, with no default, the edge past the body)
        // comes last; the scan follows every edge of the switch.
        let flags = if default_live { edge::BOTH } else { edge::SCAN };
        let de = b.edge(src, flags);
        b.join(j.brk);
        match f.default_block {
            Some(t) => b.resolve(&[de], t),
            None => b.dangling.push(de),
        }
        self.cfg_scope_end(mark, b);
    }

    fn cfg_try(&mut self, s: StmtId, b: &mut Builder) {
        let idx = b.cfg.tries.len() as u32;
        let outer = b.try_stack.last().map(|t| t.0);
        b.cfg.tries.push(TryInfo { stmt: s, dispatch: BlockId::EXIT, outer });
        b.try_stack.push((idx, Vec::new()));
        self.cfg_stmt(self.ast.stmts.try_block(s), b);
        let (_, throws) = b.try_stack.pop().expect("try frame");
        let out = b.take_pos();
        let dispatch = b.new_block();
        b.cfg.blocks[dispatch.idx()].term = Term::Try(s);
        b.cfg.tries[idx as usize].dispatch = dispatch;
        b.resolve(&throws, dispatch);
        let handlers: Vec<StmtId> = self.ast.lists.iter(self.ast.stmts.try_handlers(s)).collect();
        let mut hedges = Vec::with_capacity(handlers.len());
        for _ in &handlers {
            hedges.push(b.edge(dispatch, edge::BOTH));
        }
        if !self.has_catch_all(s) {
            if let Some(t) = b.try_stack.last_mut() {
                let e = b.cfg.edges.len() as u32;
                b.cfg.edges.push(Edge { from: dispatch, to: BlockId::PENDING, flags: edge::BOTH });
                t.1.push(e);
            } else {
                let e = b.edge(dispatch, edge::BOTH | edge::ABNORMAL);
                b.cfg.edges[e as usize].to = BlockId::EXIT;
            }
        }
        let mut outs = out;
        for (h, e) in handlers.into_iter().zip(hedges) {
            b.set_pos(vec![e]);
            let blk = b.open();
            b.cfg.blocks[blk.idx()].label = Some(h);
            let mark = b.dtors.len();
            if let Some(v) = self.ast.stmts.catch_var(h) {
                self.cfg_scope_var(v, b);
            }
            self.cfg_stmt(self.ast.stmts.catch_block(h), b);
            self.cfg_scope_end(mark, b);
            outs.extend(b.take_pos());
        }
        b.set_pos(outs);
    }

    #[allow(clippy::too_many_lines, reason = "one arm per expression kind")]
    fn cfg_expr(&mut self, e: ExprId, b: &mut Builder) {
        if !b.ok() {
            return;
        }
        if !b.req.probes.is_empty() && b.req.probes.contains(&e) {
            let p = b.pos();
            b.cfg.probes.push((e, p));
        }
        let ex = &self.ast.exprs;
        let kind = ex.get(e).kind();
        match kind {
            // clang `CFGBuilder::VisitUnaryExprOrTypeTraitExpr` adds the
            // node but walks only a VLA bound: the operand of `sizeof`
            // and `alignof` is unevaluated.
            ExprKind::LambdaExpr | ExprKind::UnaryExprOrTypeTraitExpr => b.push(Elem::Expr(e)),
            // Implicit initializers have no CFG element of their own.
            ExprKind::ImplicitValueInitExpr | ExprKind::NoInitExpr => {}
            ExprKind::CXXThrowExpr => {
                self.cfg_kids(e, b);
                b.push(Elem::Expr(e));
                let blk = b.open();
                let try_ = b.try_stack.last().map(|t| t.0);
                b.cfg.throws.push(Throw { expr: e, block: blk, try_ });
                match try_ {
                    Some(_) => {
                        let ed = b.jump(Term::None, edge::BOTH);
                        b.try_stack.last_mut().expect("try frame").1.push(ed);
                    }
                    None => b.jump_to_exit(edge::BOTH | edge::FAKE),
                }
            }
            ExprKind::ConditionalOperator => {
                // `kids` is the walk's operand stack: read above `base`.
                let base = b.kids.len();
                ex.children_into(e, &self.ast.lists, &mut b.kids);
                let [c, t, f] = [b.kids[base], b.kids[base + 1], b.kids[base + 2]];
                b.kids.truncate(base);
                self.cfg_expr(c, b);
                let kv = self.cfg_truth(c, c, b);
                let src = b.terminate(Term::Cond(e));
                let te = b.edge(src, kv.flags(true, false));
                let fe = b.edge(src, kv.flags(false, false));
                b.temp_tok(TempTok::Arms);
                b.temp_tok(TempTok::Open(kv.base));
                b.set_pos(vec![te]);
                self.cfg_expr(t, b);
                b.temp_tok(TempTok::Close);
                let tout = b.take_pos();
                b.temp_tok(TempTok::Open(kv.base.map(|v| !v)));
                b.set_pos(vec![fe]);
                self.cfg_expr(f, b);
                b.temp_tok(TempTok::Close);
                b.join(tout);
                b.push(Elem::Expr(e));
            }
            ExprKind::BinaryConditionalOperator => {
                // clang `VisitConditionalOperator` for `a ?: b`: the
                // common expression and the condition (when not the bare
                // opaque value) end the condition block; its true edge
                // reaches the join directly, its false edge runs `b`.
                let [common, c, _, f] = ex.binary_conditional_operands(e, &self.ast.lists);
                let bare = ex.get(c).kind() == ExprKind::OpaqueValueExpr;
                self.cfg_expr(common, b);
                if !bare {
                    self.cfg_expr(c, b);
                }
                let kv = self.cfg_truth(c, c, b);
                let src = b.terminate(Term::Cond(e));
                let te = b.edge(src, kv.flags(true, false));
                let fe = b.edge(src, kv.flags(false, false));
                b.temp_tok(TempTok::Arms);
                b.temp_tok(TempTok::Open(kv.base));
                b.temp_tok(TempTok::Close);
                b.temp_tok(TempTok::Open(kv.base.map(|v| !v)));
                b.set_pos(vec![fe]);
                self.cfg_expr(f, b);
                b.temp_tok(TempTok::Close);
                b.join(vec![te]);
                b.push(Elem::Expr(e));
            }
            ExprKind::BinaryOperator
                if matches!(ex.binary_op(e), BinaryOp::LAnd | BinaryOp::LOr) =>
            {
                // clang `VisitBinaryOperator`: the operators of a nested
                // `&&`/`||` share the join block, which holds the value.
                let o = self.cfg_logical_cond(e, None, e, true, b);
                let mut all = o.t;
                all.extend(o.f);
                b.set_pos(all);
                b.push(Elem::Expr(e));
            }
            // clang `CFGBuilder::VisitExprWithCleanups` visits only the
            // subexpression; the temporaries' destructors follow it.
            ExprKind::ExprWithCleanups => {
                let mark = b.temps.len();
                let saved = b.extended;
                b.extended = 0;
                b.cleanups += 1;
                self.cfg_kids(e, b);
                b.cleanups -= 1;
                b.extended = saved;
                if b.temps.len() > mark {
                    let toks = b.temps.split_off(mark);
                    if b.has_pos() {
                        self.cfg_temp_dtors(&toks, Some(true), b);
                    }
                }
            }
            ExprKind::MaterializeTemporaryExpr => {
                let ext = ex.lifetime_extended_temporary(e).is_some();
                b.extended += u32::from(ext);
                self.cfg_kids(e, b);
                b.extended -= u32::from(ext);
                b.push(Elem::Expr(e));
            }
            ExprKind::CXXBindTemporaryExpr => {
                self.cfg_kids(e, b);
                b.push(Elem::Expr(e));
                if b.extended == 0 {
                    b.temp_tok(TempTok::Temp(e));
                }
            }
            ExprKind::StmtExpr => {
                self.cfg_stmt(ex.stmt_expr_body(e), b);
                b.push(Elem::Expr(e));
            }
            _ => {
                self.cfg_kids(e, b);
                // clang `isValidDeadStmt`: a comma is never a dead root.
                let comma = kind == ExprKind::BinaryOperator
                    && self.ast.exprs.binary_op(e) == BinaryOp::Comma;
                if !comma {
                    b.push(Elem::Expr(e));
                } else if b.req.uninit {
                    b.cfg.commas.push(e);
                }
                if b.has_pos() && self.is_noreturn_call(e) {
                    Self::cfg_noreturn(b);
                }
            }
        }
    }

    /// clang `VisitForTemporaryDtors`, forwards: the destructors of the
    /// temporaries `toks` (one operand list, construction order) in
    /// reverse order; a conditional operand's run under a
    /// `TemporaryDtorsBranch` on its first temporary. `known`: whether
    /// the operand list is known to have been evaluated.
    fn cfg_temp_dtors(&mut self, toks: &[TempTok], known: Option<bool>, b: &mut Builder) {
        // The top-level items: a temporary (`i..i+1`), an operand
        // (`Open..=Close`), or `Arms` and two operands.
        let mut items: Vec<(usize, usize)> = Vec::new();
        let mut i = 0;
        while i < toks.len() {
            let start = i;
            let groups = if matches!(toks[i], TempTok::Arms) {
                i += 1;
                2
            } else {
                usize::from(matches!(toks[i], TempTok::Open(_)))
            };
            if groups == 0 {
                i += 1;
            }
            for _ in 0..groups {
                let mut depth = 0u32;
                loop {
                    match toks[i] {
                        TempTok::Open(_) => depth += 1,
                        TempTok::Close => depth -= 1,
                        _ => {}
                    }
                    i += 1;
                    if depth == 0 {
                        break;
                    }
                }
            }
            items.push((start, i));
        }
        for &(lo, hi) in items.iter().rev() {
            if !b.has_pos() {
                return;
            }
            match toks[lo] {
                TempTok::Temp(t) => {
                    b.push(Elem::TempDtor(t));
                    let d = self.ast.exprs.bound_destructor(t);
                    if self.ast.is_noreturn_fn(d) {
                        Self::cfg_noreturn(b);
                    }
                }
                TempTok::Open(k) => {
                    let inner = &toks[lo + 1..hi - 1];
                    self.cfg_temp_branch(inner, both_known(known, k), None, b);
                }
                TempTok::Arms => {
                    let t_end = lo + 1 + group_len(&toks[lo + 1..hi]);
                    let (TempTok::Open(tk), TempTok::Open(fk)) = (toks[lo + 1], toks[t_end]) else {
                        continue;
                    };
                    let (tt, ft) = (&toks[lo + 2..t_end - 1], &toks[t_end + 1..hi - 1]);
                    let (tk, fk) = (both_known(known, tk), both_known(known, fk));
                    match (first_temp(tt), first_temp(ft)) {
                        // The decision on the false arm's temporary falls
                        // back to the true arm's destructors.
                        (Some(_), Some(_)) => self.cfg_temp_branch(ft, fk, Some((tt, tk)), b),
                        (Some(_), None) => self.cfg_temp_branch(tt, tk, None, b),
                        _ => {
                            self.cfg_temp_branch(ft, fk, None, b);
                            self.cfg_temp_branch(tt, tk, None, b);
                        }
                    }
                }
                // The record is balanced: no item starts with `Close`.
                TempTok::Close => {}
            }
        }
    }

    /// One conditional operand's destructors `toks` behind a decision on
    /// its first temporary (inline when it has none of its own); the
    /// decision's false edge runs `other` (the true arm of a `?:`).
    fn cfg_temp_branch(
        &mut self,
        toks: &[TempTok],
        known: Option<bool>,
        other: Option<(&[TempTok], Option<bool>)>,
        b: &mut Builder,
    ) {
        let Some(first) = first_temp(toks) else {
            self.cfg_temp_dtors(toks, known, b);
            return;
        };
        let kv = Truth { base: known, scan: known };
        let src = b.terminate(Term::TempDtor(first));
        let te = b.edge(src, kv.flags(true, false));
        let fe = b.edge(src, kv.flags(false, false));
        b.set_pos(vec![te]);
        self.cfg_temp_dtors(toks, known, b);
        let out = b.take_pos();
        b.set_pos(vec![fe]);
        if let Some((o, ok)) = other {
            self.cfg_temp_dtors(o, ok, b);
        }
        b.join(out);
    }

    fn cfg_kids(&mut self, e: ExprId, b: &mut Builder) {
        let base = b.kids.len();
        self.ast.exprs.children_into(e, &self.ast.lists, &mut b.kids);
        // clang's CFG evaluates the right operand of `=` first.
        if self.ast.exprs.get(e).kind() == ExprKind::BinaryOperator
            && self.ast.exprs.binary_op(e) == BinaryOp::Assign
        {
            b.kids[base..].reverse();
        }
        let end = b.kids.len();
        for i in base..end {
            let c = b.kids[i];
            self.cfg_expr(c, b);
        }
        b.kids.truncate(base);
    }
}

/// Builds the successor and predecessor lists from the edges that got a
/// target (successors in creation order; a switch's cases in
/// clang's order: last case first, then the default edge).
fn finalize(cfg: &mut Cfg) {
    let n = cfg.blocks.len();
    let mut count = vec![0u32; n + 1];
    for e in cfg.edges.iter().filter(|e| e.to != BlockId::PENDING) {
        count[e.from.idx() + 1] += 1;
    }
    for i in 0..n {
        count[i + 1] += count[i];
    }
    cfg.succs.clear();
    cfg.succs.resize(count[n] as usize, Adj { block: BlockId::EXIT, flags: 0 });
    let mut fill = count.clone();
    for e in cfg.edges.iter().filter(|e| e.to != BlockId::PENDING) {
        let at = &mut fill[e.from.idx()];
        cfg.succs[*at as usize] = Adj { block: e.to, flags: e.flags };
        *at += 1;
    }
    for (i, blk) in cfg.blocks.iter_mut().enumerate() {
        blk.succs = count[i]..count[i + 1];
        if matches!(blk.term, Term::Switch(_)) && blk.succs.len() > 1 {
            let r = blk.succs.start as usize..blk.succs.end as usize - 1;
            cfg.succs[r].reverse();
        }
    }
    let mut count = vec![0u32; n + 1];
    for e in cfg.edges.iter().filter(|e| e.to != BlockId::PENDING) {
        count[e.to.idx() + 1] += 1;
    }
    for i in 0..n {
        count[i + 1] += count[i];
    }
    cfg.preds.clear();
    cfg.preds.resize(count[n] as usize, Adj { block: BlockId::EXIT, flags: 0 });
    let mut fill = count.clone();
    // Edges in creation order are not sorted by source; walk the blocks.
    for (from, blk) in cfg.blocks.iter().enumerate() {
        for a in &cfg.succs[blk.succs.start as usize..blk.succs.end as usize] {
            let at = &mut fill[a.block.idx()];
            cfg.preds[*at as usize] = Adj { block: BlockId(from as u32), flags: a.flags };
            *at += 1;
        }
    }
    for (i, blk) in cfg.blocks.iter_mut().enumerate() {
        blk.preds = count[i]..count[i + 1];
    }
}
