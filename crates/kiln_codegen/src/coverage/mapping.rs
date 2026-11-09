//! clang 19 `CoverageMappingGen`'s region builders (WP-1003,
//! spec/codegen.md "Instrumentation"): `CounterCoverageMappingBuilder`
//! walks a function body and turns the counters of [`super::pgo`] into
//! code, gap, skipped, branch and expansion regions;
//! `EmptyCoverageMappingBuilder` gives an unused function its one zero
//! region.
//!
//! The port keeps clang's algorithm statement for statement, including
//! its order of `SourceRegions` (the writer's sort is stable, so the
//! encoded bytes depend on it). Locations follow clang's model: a
//! "file ID" is one inclusion of a file or one macro expansion entry
//! ([`SourceManager::entry_bounds`]). Where kiln's AST does not record a
//! location clang has (the `)` after a condition, a declaration
//! statement's `;`, a case's `:`), it is recovered by re-lexing the
//! spelling buffer from the statement's first token.

use kiln_ast::decl::{DeclKind, Defaulted};
use kiln_ast::expr::{BinaryOp, ExprKind};
use kiln_ast::stmt::{IfKind, StmtKind};
use kiln_ast::{Ast, DeclId, ExprId, StmtId};
use kiln_lex::{LexOptions, Lexer, Punct, TokKind};
use kiln_source::fx::{FxHashMap, FxHashSet};
use kiln_source::symbol::Interner;
use kiln_source::{ExpansionKind, FileId, Loc, SourceManager};

use super::encode::{CExpr, Counter, ExprBuilder, Kind, MapRegion};
use super::pgo::{Key, RegionCounters, ignore_parens, is_instrumented_condition};
use super::skipped::{SkipKind, SkippedRange};

/// Bound on every location walk (include and expansion chains are far
/// shorter; the bound only guards against a malformed chain).
const WALK: usize = 4096;

/// The location queries of `CoverageMappingBuilder`.
#[derive(Clone, Copy)]
pub(crate) struct Locs<'a> {
    pub sm: &'a SourceManager,
    pub interner: &'a Interner,
}

impl<'a> Locs<'a> {
    /// clang `getFileID`: the bounds of the entry holding `l`.
    fn fid(self, l: Loc) -> Option<(Loc, Loc)> {
        if l.is_valid() { self.sm.entry_bounds(l) } else { None }
    }

    /// `isWrittenInSameFile` (the same file ID; two invalid locations
    /// compare equal, as clang's null `FileID`s do).
    fn same(self, a: Loc, b: Loc) -> bool {
        self.fid(a) == self.fid(b)
    }

    /// `getImmediateExpansionRange`: a macro argument's is its parameter's
    /// location, a function-like macro's ends at its `)`.
    fn imm_exp(self, l: Loc) -> (Loc, Loc) {
        match self.sm.expansion_info(l) {
            Some(i) => {
                let lo = i.expansion.lo;
                let hi = match i.kind {
                    ExpansionKind::Macro { name } if name.hi != i.expansion.hi => {
                        Loc::from_raw(i.expansion.hi.to_raw().saturating_sub(1))
                    }
                    _ => lo,
                };
                (lo, hi)
            }
            None => (Loc::INVALID, Loc::INVALID),
        }
    }

    fn is_fn_macro(self, l: Loc) -> bool {
        self.sm.expansion_info(l).is_some_and(
            |i| matches!(i.kind, ExpansionKind::Macro { name } if name.hi != i.expansion.hi),
        )
    }

    fn is_macro_arg(self, l: Loc) -> bool {
        self.sm.expansion_info(l).is_some_and(|i| matches!(i.kind, ExpansionKind::MacroArg))
    }

    fn buffer_name(self, l: Loc) -> Option<&'a str> {
        let (f, _) = self.sm.offset_of(self.sm.spelling_loc(l))?;
        Some(self.sm.file_name(f))
    }

    fn in_builtin(self, l: Loc) -> bool {
        l.is_macro() && self.buffer_name(l) == Some("<built-in>")
    }

    fn in_scratch(self, l: Loc) -> bool {
        l.is_macro() && self.buffer_name(l) == Some("<scratch space>")
    }

    /// `getIncludeOrExpansionLoc` (accepting scratch space).
    fn inc_or_exp(self, l: Loc) -> Loc {
        if !l.is_valid() {
            Loc::INVALID
        } else if l.is_macro() {
            self.imm_exp(l).0
        } else {
            self.sm.include_loc(l)
        }
    }

    /// `getNonScratchExpansionLoc`.
    fn non_scratch(self, mut l: Loc) -> (Loc, Option<Loc>) {
        let mut end = None;
        for _ in 0..WALK {
            if !self.in_scratch(l) {
                break;
            }
            let (b, e) = self.imm_exp(l);
            l = b;
            end = Some(e);
        }
        (l, end)
    }

    fn start_of(self, l: Loc) -> Loc {
        self.fid(l).map_or(Loc::INVALID, |b| b.0)
    }

    fn end_of(self, l: Loc) -> Loc {
        self.fid(l).map_or(Loc::INVALID, |b| b.1)
    }

    /// `isNestedIn`.
    fn nested_in(self, mut l: Loc, parent: Option<(Loc, Loc)>) -> bool {
        for _ in 0..WALK {
            l = self.inc_or_exp(l);
            if !l.is_valid() {
                return false;
            }
            if self.fid(l) == parent {
                return true;
            }
        }
        false
    }

    /// `locationDepth`.
    fn depth(self, mut l: Loc) -> usize {
        let mut d = 0;
        while l.is_valid() && d < WALK {
            l = self.inc_or_exp(l);
            d += 1;
        }
        d
    }

    /// `getPreciseTokenLocEnd`: `l` plus the length of the token spelled
    /// there.
    pub(crate) fn tok_end(self, l: Loc) -> Loc {
        if !l.is_valid() {
            return l;
        }
        let Some((file, off, base)) = self.spelled(l) else { return l };
        let len = self.sm.text(file).len() as u32;
        let mut lx = Lexer::new_in_range(
            self.sm,
            self.interner,
            file,
            off..len,
            base,
            LexOptions::default(),
        );
        let t = lx.next();
        // clang `Lexer::MeasureTokenLength`: zero at whitespace (or
        // anything that does not start a token right here).
        if t.span.lo.to_raw() != base.to_raw() + off {
            return l;
        }
        let n = t.span.hi.to_raw().saturating_sub(t.span.lo.to_raw());
        l.with_offset(n)
    }

    /// The file, offset and file base location of `l`'s spelling, when
    /// the offset can start a lexer.
    fn spelled(self, l: Loc) -> Option<(FileId, u32, Loc)> {
        let sp = self.sm.spelling_loc(l);
        let (file, off) = self.sm.offset_of(sp)?;
        let text = self.sm.text(file);
        if off as usize > text.len() || !text.is_char_boundary(off as usize) {
            return None;
        }
        Some((file, off, Loc::from_raw(sp.to_raw() - off)))
    }

    /// Re-lexes from the token at `from` and returns the location (in
    /// `from`'s entry) of the first token `pick` accepts; `pick` sees
    /// each token with the bracket depth before it and its index.
    fn scan(self, from: Loc, mut pick: impl FnMut(TokKind, u32, usize) -> bool) -> Loc {
        let Some((file, off, base)) = self.spelled(from) else { return Loc::INVALID };
        let len = self.sm.text(file).len() as u32;
        let mut lx =
            Lexer::new_in_range(self.sm, self.interner, file, off..len, base, LexOptions::default());
        let sp = base.with_offset(off);
        let mut depth = 0u32;
        for i in 0..1_000_000usize {
            let t = lx.next();
            if t.kind == TokKind::Eof {
                break;
            }
            if pick(t.kind, depth, i) {
                let d = t.span.lo.to_raw().saturating_sub(sp.to_raw());
                let l = from.with_offset(d);
                return if self.same(from, l) { l } else { Loc::INVALID };
            }
            match t.kind {
                TokKind::Punct(Punct::LParen | Punct::LBrace | Punct::LSquare) => depth += 1,
                TokKind::Punct(Punct::RParen | Punct::RBrace | Punct::RSquare) => {
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
        }
        Loc::INVALID
    }

    /// The `= default` of the function declared at `from`: the start of
    /// the token before `=` and the last character of `default` (clang
    /// gives an explicitly defaulted function a body at its declarator's
    /// end, which is that last character in a class, `SetRangeEnd`).
    fn eq_default(self, from: Loc) -> Option<(Loc, Loc)> {
        let (file, off, base) = self.spelled(from)?;
        let len = self.sm.text(file).len() as u32;
        let mut lx =
            Lexer::new_in_range(self.sm, self.interner, file, off..len, base, LexOptions::default());
        let sp = base.with_offset(off);
        let at = |l: Loc| from.with_offset(l.to_raw().saturating_sub(sp.to_raw()));
        let (mut depth, mut prev, mut before_eq) = (0u32, None, None);
        for _ in 0..100_000usize {
            let t = lx.next();
            match t.kind {
                TokKind::Eof | TokKind::Punct(Punct::Semi | Punct::LBrace) if depth == 0 => {
                    return None;
                }
                TokKind::Eof => return None,
                TokKind::Punct(Punct::LParen | Punct::LBrace | Punct::LSquare) => depth += 1,
                TokKind::Punct(Punct::RParen | Punct::RBrace | Punct::RSquare) => {
                    depth = depth.saturating_sub(1);
                }
                TokKind::Punct(Punct::Equal) if depth == 0 => {
                    before_eq = prev;
                    prev = Some(t.span.lo);
                    continue;
                }
                _ if before_eq.is_some() && self.sm.span_text(t.span) == Some("default") => {
                    let last = t.span.hi.to_raw().saturating_sub(1);
                    let (b, d) = (at(before_eq?), at(Loc::from_raw(last)));
                    return (self.same(from, b) && self.same(from, d)).then_some((b, d));
                }
                _ => {}
            }
            before_eq = None;
            prev = Some(t.span.lo);
        }
        None
    }

    /// The `)` closing the first `(` after `from`.
    fn rparen(self, from: Loc) -> Loc {
        let mut seen = false;
        self.scan(from, |k, d, _| match k {
            TokKind::Punct(Punct::LParen) if d == 0 => {
                seen = true;
                false
            }
            TokKind::Punct(Punct::RParen) => seen && d == 1,
            _ => false,
        })
    }

    /// Spelling line and column (`SpellingRegion`).
    fn lc(self, l: Loc) -> Option<(u32, u32)> {
        let x = self.sm.line_col(self.sm.spelling_loc(l))?;
        Some((x.line, x.col))
    }

    fn in_order(self, a: Loc, b: Loc) -> bool {
        matches!((self.lc(a), self.lc(b)), (Some(x), Some(y)) if x <= y)
    }

    fn sys(self, l: Loc) -> bool {
        self.sm.is_in_system_header(self.sm.spelling_loc(l))
    }
}

/// A statement or expression (clang's `const Stmt *`; an expression
/// statement is its expression).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum N {
    S(StmtId),
    E(ExprId),
}

/// clang `SourceMappingRegion`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Reg {
    count: Counter,
    fcount: Option<Counter>,
    gap: bool,
    skipped: bool,
    start: Option<Loc>,
    end: Option<Loc>,
}

impl Reg {
    fn new(count: Counter, start: Option<Loc>, end: Option<Loc>) -> Self {
        Reg { count, fcount: None, gap: false, skipped: false, start, end }
    }
}

/// Statement locations shared by both builders.
#[derive(Clone, Copy)]
struct Shape<'a> {
    ast: &'a Ast,
    l: Locs<'a>,
}

impl Shape<'_> {
    fn norm(self, s: StmtId) -> N {
        let st = &self.ast.stmts;
        if st.get(s).kind() == StmtKind::ExprStmt { N::E(st.expr_stmt_expr(s)) } else { N::S(s) }
    }

    /// `S->getBeginLoc()`.
    fn begin(self, mut n: N) -> Loc {
        let st = &self.ast.stmts;
        for _ in 0..WALK {
            match n {
                N::E(e) => return self.ast.begin_loc(e),
                N::S(s) => match st.get(s).kind() {
                    StmtKind::CoroutineBodyStmt => n = self.norm(st.coroutine_body(s).body),
                    _ => return st.get(s).loc(),
                },
            }
        }
        Loc::INVALID
    }

    /// `S->getEndLoc()`: the first location of the last token.
    fn end_raw(self, mut n: N) -> Loc {
        let st = &self.ast.stmts;
        for _ in 0..WALK {
            let s = match n {
                N::E(e) => return self.ast.end_loc(e),
                N::S(s) => s,
            };
            let loc = st.get(s).loc();
            let next = match st.get(s).kind() {
                StmtKind::CompoundStmt => return st.rbrace(s),
                StmtKind::DeclStmt => {
                    return self.l.scan(loc, |k, d, _| d == 0 && k == TokKind::Punct(Punct::Semi));
                }
                StmtKind::IfStmt => {
                    let d = st.if_data(s);
                    d.els.or(d.then)
                }
                StmtKind::SwitchStmt => Some(st.switch_data(s).body),
                StmtKind::WhileStmt | StmtKind::ForStmt => st.loop_data(s).body,
                StmtKind::CXXForRangeStmt => Some(st.range_for_data(s).body),
                StmtKind::DoStmt => return st.do_rparen(s),
                StmtKind::ReturnStmt => return st.return_value(s).map_or(loc, |e| self.ast.end_loc(e)),
                StmtKind::CoreturnStmt => {
                    return st.coreturn_operand(s).map_or(loc, |e| self.ast.end_loc(e));
                }
                StmtKind::LabelStmt | StmtKind::AttributedStmt => Some(StmtId::new(st.get(s).b())),
                StmtKind::CaseStmt => Some(st.case_data(s).sub),
                StmtKind::DefaultStmt => Some(st.default_sub(s)),
                StmtKind::GotoStmt => return self.l.scan(loc, |_, _, i| i == 1),
                StmtKind::IndirectGotoStmt => return self.ast.end_loc(st.indirect_goto_target(s)),
                StmtKind::CXXTryStmt => self.ast.lists.iter::<StmtId>(st.try_handlers(s)).last(),
                StmtKind::CXXCatchStmt => Some(st.catch_block(s)),
                StmtKind::CoroutineBodyStmt => Some(st.coroutine_body(s).body),
                StmtKind::GCCAsmStmt | StmtKind::ContractAssertStmt => return self.l.rparen(loc),
                _ => return loc,
            };
            match next {
                Some(x) => n = self.norm(x),
                None => return loc,
            }
        }
        Loc::INVALID
    }

    /// `getStart` / the location `getEnd` measures: out of macro
    /// arguments and builtin macros.
    fn unarg(self, mut l: Loc) -> Loc {
        for _ in 0..WALK {
            if !(self.l.is_macro_arg(l) || self.l.in_builtin(l)) {
                break;
            }
            l = self.l.imm_exp(l).0;
        }
        l
    }

    fn start(self, n: N) -> Loc {
        self.unarg(self.begin(n))
    }

    fn end(self, n: N) -> Loc {
        self.l.tok_end(self.unarg(self.end_raw(n)))
    }
}

/// The coverage file table of the module (clang
/// `CoverageMappingModuleGen`'s `FileEntries` and `CoverageSourceInfo`).
pub(crate) trait Files {
    /// `CVM.getFileID`: the global index of `file` (from 1).
    fn global_id(&mut self, file: FileId) -> u32;
    /// The skipped ranges of the file entry starting at `base`.
    fn skipped(&mut self, file: FileId, base: Loc) -> &[SkippedRange];
}

/// clang `CounterCoverageMappingBuilder`.
pub(crate) struct Builder<'a, 'f> {
    sh: Shape<'a>,
    l: Locs<'a>,
    rc: &'a RegionCounters,
    fold: &'f mut dyn FnMut(ExprId) -> Option<i128>,
    eb: ExprBuilder,
    stack: Vec<Reg>,
    regions: Vec<Reg>,
    added: FxHashSet<(Loc, Loc, bool)>,
    bc: Vec<(Counter, Counter)>,
    mrl: Loc,
    has_term: bool,
    gap_rc: Counter,
    leaf: FxHashSet<ExprId>,
    kids: Vec<ExprId>,
}

impl<'a, 'f> Builder<'a, 'f> {
    pub(crate) fn new(
        ast: &'a Ast,
        l: Locs<'a>,
        rc: &'a RegionCounters,
        fold: &'f mut dyn FnMut(ExprId) -> Option<i128>,
    ) -> Self {
        Builder {
            sh: Shape { ast, l },
            l,
            rc,
            fold,
            eb: ExprBuilder::default(),
            stack: Vec::new(),
            regions: Vec::new(),
            added: FxHashSet::default(),
            bc: Vec::new(),
            mrl: Loc::INVALID,
            has_term: false,
            gap_rc: Counter::Zero,
            leaf: FxHashSet::default(),
            kids: Vec::new(),
        }
    }

    fn ast(&self) -> &'a Ast {
        self.sh.ast
    }

    fn rcs(&self, s: StmtId) -> Counter {
        Counter::Ref(self.rc.map.get(&Key::Stmt(s)).copied().unwrap_or(0))
    }

    fn rce(&self, e: ExprId) -> Counter {
        Counter::Ref(self.rc.map.get(&Key::Expr(e)).copied().unwrap_or(0))
    }

    fn start(&self, n: N) -> Loc {
        self.sh.start(n)
    }

    fn end(&self, n: N) -> Loc {
        self.sh.end(n)
    }

    fn add(&mut self, a: Counter, b: Counter) -> Counter {
        self.eb.add(a, b)
    }

    fn sub(&mut self, a: Counter, b: Counter) -> Counter {
        self.eb.sub(a, b)
    }

    fn top(&mut self) -> &mut Reg {
        if self.stack.is_empty() {
            self.stack.push(Reg::new(Counter::Zero, None, None));
        }
        let i = self.stack.len() - 1;
        &mut self.stack[i]
    }

    fn top_count(&mut self) -> Counter {
        self.top().count
    }

    fn emit(&mut self, r: Reg) {
        if let (Some(s), Some(e)) = (r.start, r.end) {
            self.added.insert((s, e, r.fcount.is_some()));
        }
        self.regions.push(r);
    }

    fn already(&self, s: Loc, e: Loc, branch: bool) -> bool {
        self.added.contains(&(s, e, branch))
    }

    /// `pushRegion`.
    fn push(&mut self, count: Counter, start: Option<Loc>, end: Option<Loc>, f: Option<Counter>) -> usize {
        if let Some(s) = start
            && f.is_none()
        {
            self.mrl = s;
        }
        let valid = |l: Option<Loc>| l.filter(|l| l.is_valid());
        self.stack.push(Reg { fcount: f, ..Reg::new(count, valid(start), valid(end)) });
        self.stack.len() - 1
    }

    /// `popRegions`.
    fn pop(&mut self, parent: usize) {
        let l = self.l;
        while self.stack.len() > parent {
            let Some(mut r) = self.stack.pop() else { break };
            let pend = self.stack.get(parent).and_then(|p| p.end);
            let Some(mut s) = r.start else { continue };
            let Some(mut e) = r.end.or(if parent < self.stack.len() { pend } else { None }) else {
                continue;
            };
            let branch = r.fcount.is_some();
            let (mut sd, mut ed) = (l.depth(s), l.depth(e));
            let mut ok = true;
            for _ in 0..WALK {
                if l.same(s, e) {
                    break;
                }
                let (us, ue) = (sd >= ed, ed >= sd);
                if ue {
                    let nested = l.start_of(e);
                    if !branch && !self.already(nested, e, false) {
                        self.emit(Reg::new(r.count, Some(nested), Some(e)));
                    }
                    e = l.tok_end(l.inc_or_exp(e));
                    ed = ed.saturating_sub(1);
                }
                if us {
                    let nested = l.end_of(s);
                    if !branch && !self.already(s, nested, false) {
                        self.emit(Reg::new(r.count, Some(s), Some(nested)));
                    }
                    s = l.inc_or_exp(s);
                    sd = sd.saturating_sub(1);
                }
                if !s.is_valid() || !e.is_valid() {
                    ok = false;
                    break;
                }
            }
            if !ok || !l.same(s, e) {
                continue;
            }
            r.start = Some(s);
            r.end = Some(e);
            if !branch {
                self.mrl = e;
                if s == l.start_of(s) && e == l.end_of(e) {
                    self.mrl = l.inc_or_exp(e);
                }
            }
            self.emit(r);
        }
    }

    /// `propagateCounts`.
    fn propagate(&mut self, top: Counter, n: N, visit: bool) -> Counter {
        let s = self.start(n);
        let e = self.end(n);
        let i = self.push(top, Some(s), Some(e), None);
        if visit {
            self.visit(n);
        }
        let exit = self.top_count();
        self.pop(i);
        if s != self.sh.begin(n) {
            self.mrl = e;
        }
        exit
    }

    fn folds(&mut self, e: ExprId) -> Option<i128> {
        (self.fold)(e)
    }

    /// `createBranchRegion`.
    fn branch(&mut self, c: Option<ExprId>, t: Counter, f: Counter) {
        let Some(c) = c else { return };
        let ast = self.ast();
        if !(is_instrumented_condition(ast, c) || self.leaf.contains(&strip_cond(ast, c))) {
            return;
        }
        let (s, e) = (self.start(N::E(c)), self.end(N::E(c)));
        let i = if self.folds(c).is_some() {
            self.push(Counter::Zero, Some(s), Some(e), Some(Counter::Zero))
        } else {
            self.push(t, Some(s), Some(e), Some(f))
        };
        self.pop(i);
    }

    /// `adjustForOutOfOrderTraversal`.
    fn adjust_ooo(&mut self, end: Loc) {
        self.mrl = end;
        let l = self.l;
        let top = *self.top();
        if top.end.is_some()
            && self.mrl == l.end_of(self.mrl)
            && self.already(l.start_of(self.mrl), self.mrl, top.fcount.is_some())
        {
            self.mrl = l.inc_or_exp(self.mrl);
        }
    }

    /// `handleFileExit`.
    fn file_exit(&mut self, new: Loc) {
        let l = self.l;
        if !new.is_valid() || l.same(self.mrl, new) {
            return;
        }
        let mut lca = new;
        let mut pf = l.fid(lca);
        let mut found = false;
        for _ in 0..WALK {
            if l.nested_in(self.mrl, pf) {
                found = true;
                break;
            }
            lca = l.inc_or_exp(lca);
            if !lca.is_valid() || l.same(lca, self.mrl) {
                self.mrl = new;
                return;
            }
            pf = l.fid(lca);
        }
        if !found {
            self.mrl = new;
            return;
        }
        let mut starts: FxHashSet<Loc> = FxHashSet::default();
        let mut pc = None;
        let mut pushed = Vec::new();
        for i in (0..self.stack.len()).rev() {
            let r = self.stack[i];
            let Some(mut loc) = r.start else { continue };
            if !l.nested_in(loc, pf) {
                pc = Some(r.count);
                break;
            }
            for _ in 0..WALK {
                if l.fid(loc) == pf || !loc.is_valid() {
                    break;
                }
                if starts.insert(loc) {
                    let mut x = Reg::new(r.count, Some(loc), Some(l.end_of(loc)));
                    if r.fcount.is_some() {
                        // clang passes `isBranch()` as the gap flag.
                        x.fcount = r.fcount;
                        x.gap = true;
                    }
                    pushed.push(x);
                }
                loc = l.inc_or_exp(loc);
            }
            self.stack[i].start = Some(l.tok_end(loc)).filter(|x| x.is_valid());
        }
        for x in pushed {
            self.emit(x);
        }
        if let Some(pc) = pc {
            let mut loc = self.mrl;
            for _ in 0..WALK {
                if !l.nested_in(loc, pf) {
                    break;
                }
                let fs = l.start_of(loc);
                if starts.insert(fs) {
                    self.emit(Reg::new(pc, Some(fs), Some(l.end_of(loc))));
                }
                loc = l.inc_or_exp(loc);
            }
        }
        self.mrl = new;
    }

    /// `extendRegion`.
    fn extend(&mut self, n: N) {
        let s = self.start(n);
        self.file_exit(s);
        let t = self.top();
        if t.start.is_none() && s.is_valid() {
            t.start = Some(s);
        }
    }

    /// `terminateRegion`.
    fn terminate(&mut self, n: N) {
        self.extend(n);
        let e = self.end(n);
        let t = self.top();
        if t.end.is_none() && e.is_valid() {
            t.end = Some(e);
        }
        self.push(Counter::Zero, None, None, None);
        self.has_term = true;
    }

    /// `findGapAreaBetween`.
    fn gap_between(&self, mut a: Loc, mut b: Loc) -> Option<(Loc, Loc)> {
        let l = self.l;
        if !a.is_valid() || !b.is_valid() {
            return None;
        }
        if a.is_macro() && l.is_fn_macro(a) {
            a = l.imm_exp(a).1;
        }
        let (mut sd, mut ed) = (l.depth(a), l.depth(b));
        for _ in 0..WALK {
            if l.same(a, b) {
                break;
            }
            let (us, ue) = (sd >= ed, ed >= sd);
            if ue {
                b = l.inc_or_exp(b);
                ed = ed.saturating_sub(1);
            }
            if us {
                a = l.tok_end(l.inc_or_exp(a));
                sd = sd.saturating_sub(1);
            }
            if !a.is_valid() || !b.is_valid() {
                return None;
            }
        }
        a = l.tok_end(a);
        if a.is_macro() || b.is_macro() || !l.same(a, b) || !l.in_order(a, b) {
            return None;
        }
        Some((a, b))
    }

    /// `fillGapAreaWithCount`.
    fn fill_gap(&mut self, gap: Option<(Loc, Loc)>, c: Counter) {
        let Some((s, e)) = gap else { return };
        if s == e {
            return;
        }
        self.file_exit(s);
        let i = self.push(c, Some(s), Some(e), None);
        self.top().gap = true;
        self.file_exit(e);
        self.pop(i);
    }

    /// `findAreaStartingFromTo` + `markSkipped`.
    fn mark_skipped(&mut self, mut a: Loc, mut b: Loc) {
        let l = self.l;
        if !a.is_valid() || !b.is_valid() {
            return;
        }
        if a.is_macro() && l.is_fn_macro(a) {
            a = l.imm_exp(a).0;
        }
        let (mut sd, mut ed) = (l.depth(a), l.depth(b));
        for _ in 0..WALK {
            if l.same(a, b) {
                break;
            }
            let (us, ue) = (sd >= ed, ed >= sd);
            if ue {
                b = l.inc_or_exp(b);
                ed = ed.saturating_sub(1);
            }
            if us {
                a = l.inc_or_exp(a);
                sd = sd.saturating_sub(1);
            }
            if !a.is_valid() || !b.is_valid() {
                return;
            }
        }
        if a.is_macro() || b.is_macro() || !l.same(a, b) || !l.in_order(a, b) || a == b {
            return;
        }
        self.file_exit(a);
        let i = self.push(Counter::Zero, Some(a), Some(b), None);
        self.top().skipped = true;
        self.file_exit(b);
        self.pop(i);
    }

    // ----- the function ---------------------------------------------------

    /// `VisitDecl` of the function definition `d`.
    pub(crate) fn function(&mut self, d: DeclId) {
        let ast = self.ast();
        let Some(fd) = ast.decls.function(d).copied() else { return };
        let Some(body) = fd.body else { return };
        let bn = self.sh.norm(body);
        if self.l.sys(self.start(bn)) {
            return;
        }
        let bcount = self.rcs(body);
        let kind = ast.decls.get(d).kind();
        let method = matches!(
            kind,
            DeclKind::CXXMethod
                | DeclKind::CXXConstructor
                | DeclKind::CXXDestructor
                | DeclKind::CXXConversion
        );
        let defaulted = method && fd.spec.defaulted() != Defaulted::No;
        if kind == DeclKind::CXXConstructor {
            let inits: Vec<ExprId> =
                ast.ctor_inits(d).filter(|i| i.is_written()).map(|i| i.init).collect();
            for i in inits {
                let n = N::E(i);
                if self.start(n).is_valid() && self.end(n).is_valid() {
                    self.propagate(bcount, n, true);
                }
            }
        }
        if fd.spec.defaulted() == Defaulted::Explicit
            && let Some((before_eq, kw)) = self.l.eq_default(ast.decls.get(d).loc())
        {
            let in_class = ast.decls.lexical_ctx(d) == ast.decls.get(d).ctx();
            let s = if in_class { kw } else { before_eq };
            let i = self.push(bcount, Some(s), Some(self.l.tok_end(s)), None);
            self.pop(i);
            return;
        }
        self.propagate(bcount, bn, !defaulted);
    }

    // ----- the visitors ---------------------------------------------------

    fn visit(&mut self, n: N) {
        match n {
            N::S(s) => self.visit_stmt(s),
            N::E(e) => self.visit_expr(e),
        }
    }

    fn visit_opt(&mut self, s: Option<StmtId>) {
        if let Some(s) = s {
            self.visit(self.sh.norm(s));
        }
    }

    /// The children clang's `Stmt::children()` gives a statement or
    /// expression that has no visitor of its own.
    fn children(&mut self, n: N) -> Vec<N> {
        let ast = self.ast();
        let (st, ex) = (&ast.stmts, &ast.exprs);
        match n {
            N::S(s) => match st.get(s).kind() {
                StmtKind::CompoundStmt => {
                    ast.lists.iter::<StmtId>(st.compound_body(s)).map(|x| self.sh.norm(x)).collect()
                }
                StmtKind::DeclStmt => ast
                    .lists
                    .iter::<DeclId>(st.decl_stmt_decls(s))
                    .filter_map(|d| ast.decls.var(d).and_then(|v| v.init))
                    .map(N::E)
                    .collect(),
                StmtKind::AttributedStmt => vec![self.sh.norm(StmtId::new(st.get(s).b()))],
                StmtKind::IndirectGotoStmt => vec![N::E(st.indirect_goto_target(s))],
                StmtKind::GCCAsmStmt => st.asm_operands(s).iter().map(|o| N::E(o.expr)).collect(),
                StmtKind::ContractAssertStmt => vec![N::E(st.contract_cond(s))],
                _ => Vec::new(),
            },
            N::E(e) => {
                let mut v = std::mem::take(&mut self.kids);
                v.clear();
                match ex.get(e).kind() {
                    ExprKind::StmtExpr => {
                        self.kids = v;
                        return vec![N::S(ex.stmt_expr_body(e))];
                    }
                    ExprKind::CXXRewrittenBinaryOperator => v.push(ex.rewritten_semantic(e)),
                    ExprKind::CXXDefaultArgExpr
                    | ExprKind::CXXDefaultInitExpr
                    | ExprKind::OpaqueValueExpr
                    | ExprKind::LambdaExpr
                    | ExprKind::RequiresExpr => {}
                    _ => ex.children_into(e, &ast.lists, &mut v),
                }
                let out = v.iter().map(|&k| N::E(k)).collect();
                self.kids = v;
                out
            }
        }
    }

    /// `VisitStmt`.
    fn generic(&mut self, n: N) {
        if self.sh.begin(n).is_valid() {
            self.extend(n);
        }
        let mut last: Option<N> = None;
        let mut save = self.has_term;
        self.has_term = false;
        self.gap_rc = Counter::Zero;
        for c in self.children(n) {
            if let Some(p) = last
                && self.has_term
            {
                let g = self.gap_between(self.end(p), self.start(c));
                let rc = self.gap_rc;
                self.fill_gap(g, rc);
                save = true;
                self.has_term = false;
            }
            self.visit(c);
            last = Some(c);
        }
        if save {
            self.has_term = true;
        }
        let e = self.end(n);
        self.file_exit(e);
    }

    fn visit_stmt(&mut self, s: StmtId) {
        let ast = self.ast();
        let st = &ast.stmts;
        let n = N::S(s);
        match st.get(s).kind() {
            StmtKind::ExprStmt => self.visit_expr(st.expr_stmt_expr(s)),
            StmtKind::ReturnStmt => {
                self.extend(n);
                if let Some(v) = st.return_value(s) {
                    self.visit_expr(v);
                }
                self.terminate(n);
            }
            StmtKind::CoroutineBodyStmt => {
                self.extend(n);
                let b = st.coroutine_body(s).body;
                self.visit(self.sh.norm(b));
            }
            StmtKind::CoreturnStmt => {
                if st.coreturn_is_implicit(s) {
                    return;
                }
                self.extend(n);
                if let Some(v) = st.coreturn_operand(s) {
                    self.visit_expr(v);
                }
                self.terminate(n);
            }
            StmtKind::GotoStmt => self.terminate(n),
            StmtKind::LabelStmt => {
                let start = self.start(n);
                self.file_exit(start);
                let c = self.rcs(s);
                self.push(c, Some(start), None, None);
                self.visit(self.sh.norm(StmtId::new(st.get(s).b())));
            }
            StmtKind::BreakStmt | StmtKind::ContinueStmt => {
                let brk = st.get(s).kind() == StmtKind::BreakStmt;
                let c = self.top_count();
                if let Some(&(b, k)) = self.bc.last() {
                    let v = if brk { self.add(b, c) } else { self.add(k, c) };
                    if let Some(t) = self.bc.last_mut() {
                        if brk { t.0 = v } else { t.1 = v }
                    }
                }
                self.terminate(n);
            }
            StmtKind::WhileStmt => self.while_stmt(s),
            StmtKind::DoStmt => self.do_stmt(s),
            StmtKind::ForStmt => self.for_stmt(s),
            StmtKind::CXXForRangeStmt => self.range_for(s),
            StmtKind::SwitchStmt => self.switch_stmt(s),
            StmtKind::CaseStmt | StmtKind::DefaultStmt => self.switch_case(s),
            StmtKind::IfStmt => self.if_stmt(s),
            StmtKind::CXXTryStmt => {
                self.extend(n);
                let tb = N::S(st.try_block(s));
                self.extend(tb);
                let p = self.top_count();
                self.propagate(p, tb, true);
                let hs: Vec<StmtId> = ast.lists.iter(st.try_handlers(s)).collect();
                for h in hs {
                    self.visit_stmt(h);
                }
                let c = self.rcs(s);
                self.push(c, None, None, None);
            }
            StmtKind::CXXCatchStmt => {
                let c = self.rcs(s);
                self.propagate(c, N::S(st.catch_block(s)), true);
            }
            StmtKind::CXXExpansionStmt => {}
            _ => self.generic(n),
        }
    }

    fn visit_expr(&mut self, e: ExprId) {
        let ast = self.ast();
        let ex = &ast.exprs;
        let n = N::E(e);
        match ex.get(e).kind() {
            ExprKind::CXXThrowExpr => {
                self.extend(n);
                if let Some(o) = ex.throw_operand(e) {
                    self.visit_expr(o);
                }
                self.terminate(n);
            }
            ExprKind::CoawaitExpr | ExprKind::CoyieldExpr => {
                let o = ex.coawait_operand(e, &ast.lists);
                self.visit_expr(o);
            }
            ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr | ExprKind::UserDefinedLiteral => {
                self.generic(n);
                let callee = ex.get(ex.callee(e)).ty();
                let t = &ast.types;
                let f = t.pointee(callee).unwrap_or(callee);
                if t.proto(f).is_some_and(|p| p.info().noreturn()) {
                    self.terminate(n);
                }
            }
            ExprKind::ConditionalOperator | ExprKind::BinaryConditionalOperator => self.cond_op(e),
            ExprKind::BinaryOperator if matches!(ex.binary_op(e), BinaryOp::LAnd | BinaryOp::LOr) => {
                self.logical(e, ex.binary_op(e) == BinaryOp::LAnd);
            }
            ExprKind::LambdaExpr => {}
            ExprKind::ArrayInitLoopExpr => {
                let [common, _] = ex.array_init_loop(e);
                if let Some(src) = ex.opaque_source(common) {
                    self.visit_expr(src);
                }
            }
            ExprKind::PseudoObjectExpr => {
                let syn = ex.pseudo_syntactic(e, &ast.lists);
                self.generic(N::E(syn));
            }
            ExprKind::OpaqueValueExpr => {}
            _ => self.generic(n),
        }
    }

    /// The `)` closing a statement's condition (clang `getRParenLoc`).
    fn rparen(&self, s: StmtId) -> Loc {
        self.l.rparen(self.ast().stmts.get(s).loc())
    }

    fn after_loop(&mut self, out: Counter, parent: Counter, body_term: bool) {
        if out != parent {
            self.push(out, None, None, None);
            self.gap_rc = out;
            if body_term {
                self.has_term = true;
            }
        }
    }

    fn while_stmt(&mut self, s: StmtId) {
        let st = &self.ast().stmts;
        let d = *st.loop_data(s);
        let n = N::S(s);
        self.extend(n);
        let parent = self.top_count();
        let body = self.rcs(s);
        let bn = d.body.map(|b| self.sh.norm(b));
        self.bc.push((Counter::Zero, Counter::Zero));
        let backedge = match bn {
            Some(b) => {
                self.extend(b);
                self.propagate(body, b, true)
            }
            None => body,
        };
        let (brk, cont) = self.bc.pop().unwrap_or_default();
        let body_term = self.has_term;
        self.has_term = false;
        let c1 = self.add(parent, backedge);
        let cond = self.add(c1, cont);
        if let Some(c) = d.cond {
            self.propagate(cond, N::E(c), true);
        }
        let e = self.end(n);
        self.adjust_ooo(e);
        if let Some(b) = bn {
            let g = self.gap_between(self.rparen(s), self.start(b));
            self.fill_gap(g, body);
        }
        let x = self.sub(cond, body);
        let out = self.add(brk, x);
        self.after_loop(out, parent, body_term);
        let f = self.sub(cond, body);
        self.branch(d.cond, body, f);
    }

    fn do_stmt(&mut self, s: StmtId) {
        let st = &self.ast().stmts;
        let d = *st.loop_data(s);
        let n = N::S(s);
        self.extend(n);
        let parent = self.top_count();
        let body = self.rcs(s);
        self.bc.push((Counter::Zero, Counter::Zero));
        let bn = d.body.map(|b| self.sh.norm(b));
        if let Some(b) = bn {
            self.extend(b);
        }
        let top = self.add(parent, body);
        let backedge = match bn {
            Some(b) => self.propagate(top, b, true),
            None => top,
        };
        let (brk, cont) = self.bc.pop().unwrap_or_default();
        let body_term = self.has_term;
        self.has_term = false;
        let cond = self.add(backedge, cont);
        if let Some(c) = d.cond {
            self.propagate(cond, N::E(c), true);
        }
        let x = self.sub(cond, body);
        let out = self.add(brk, x);
        if out != parent {
            self.push(out, None, None, None);
            self.gap_rc = out;
        }
        let f = self.sub(cond, body);
        self.branch(d.cond, body, f);
        if body_term {
            self.has_term = true;
        }
    }

    fn for_stmt(&mut self, s: StmtId) {
        let st = &self.ast().stmts;
        let d = *st.loop_data(s);
        let n = N::S(s);
        self.extend(n);
        self.visit_opt(d.init);
        let parent = self.top_count();
        let body = self.rcs(s);
        if d.inc.is_some() {
            self.bc.push((Counter::Zero, Counter::Zero));
        }
        self.bc.push((Counter::Zero, Counter::Zero));
        let bn = d.body.map(|b| self.sh.norm(b));
        let backedge = match bn {
            Some(b) => {
                self.extend(b);
                self.propagate(body, b, true)
            }
            None => body,
        };
        let (bbrk, bcont) = self.bc.pop().unwrap_or_default();
        let body_term = self.has_term;
        self.has_term = false;
        let (mut ibrk, mut icont) = (Counter::Zero, Counter::Zero);
        if let Some(inc) = d.inc {
            let ic = self.add(backedge, bcont);
            self.propagate(ic, N::E(inc), true);
            (ibrk, icont) = self.bc.pop().unwrap_or_default();
        }
        let c1 = self.add(parent, backedge);
        let c2 = self.add(c1, bcont);
        let cond = self.add(c2, icont);
        if let Some(c) = d.cond {
            self.propagate(cond, N::E(c), true);
            let e = self.end(n);
            self.adjust_ooo(e);
        }
        if let Some(b) = bn {
            let g = self.gap_between(self.rparen(s), self.start(b));
            self.fill_gap(g, body);
        }
        let o1 = self.add(bbrk, ibrk);
        let x = self.sub(cond, body);
        let out = self.add(o1, x);
        self.after_loop(out, parent, body_term);
        let f = self.sub(cond, body);
        self.branch(d.cond, body, f);
    }

    fn range_for(&mut self, s: StmtId) {
        let ast = self.ast();
        let d = *ast.stmts.range_for_data(s);
        let n = N::S(s);
        self.extend(n);
        self.visit_opt(d.init);
        if let Some(i) = ast.decls.var(d.range).and_then(|v| v.init) {
            self.visit_expr(i);
        }
        let parent = self.top_count();
        let body = self.rcs(s);
        self.bc.push((Counter::Zero, Counter::Zero));
        let bn = self.sh.norm(d.body);
        self.extend(bn);
        let backedge = self.propagate(body, bn, true);
        let (brk, cont) = self.bc.pop().unwrap_or_default();
        let body_term = self.has_term;
        self.has_term = false;
        let g = self.gap_between(self.rparen(s), self.start(bn));
        self.fill_gap(g, body);
        let c1 = self.add(parent, backedge);
        let lc = self.add(c1, cont);
        let x = self.sub(lc, body);
        let out = self.add(brk, x);
        self.after_loop(out, parent, body_term);
        // The implicit `__begin != __end`, at the `:`.
        if d.cond.is_some() && d.colon.is_valid() {
            let f = self.sub(lc, body);
            let e = self.l.tok_end(d.colon);
            let i = self.push(body, Some(d.colon), Some(e), Some(f));
            self.pop(i);
        }
    }

    fn switch_stmt(&mut self, s: StmtId) {
        let ast = self.ast();
        let st = &ast.stmts;
        let d = *st.switch_data(s);
        let n = N::S(s);
        self.extend(n);
        self.visit_opt(d.init);
        self.visit_expr(d.cond);
        self.bc.push((Counter::Zero, Counter::Zero));
        let bn = self.sh.norm(d.body);
        self.extend(bn);
        let compound = matches!(bn, N::S(b) if st.get(b).kind() == StmtKind::CompoundStmt);
        if compound {
            let last = ast.lists.iter::<StmtId>(st.compound_body(d.body)).last();
            if let Some(last) = last {
                let cs = self.start(bn);
                let i = self.push(Counter::Zero, Some(cs), None, None);
                self.top().gap = true;
                self.visit(bn);
                let e = self.end(self.sh.norm(last));
                for r in self.stack.iter_mut().skip(i) {
                    if r.end.is_none() && e.is_valid() {
                        r.end = Some(e);
                    }
                }
                self.pop(i);
            }
        } else {
            self.propagate(Counter::Zero, bn, true);
        }
        let (_, cont) = self.bc.pop().unwrap_or_default();
        if let Some(&(_, k)) = self.bc.last() {
            let v = self.add(k, cont);
            if let Some(t) = self.bc.last_mut() {
                t.1 = v;
            }
        }
        let parent = self.top_count();
        let exit = self.rcs(s);
        let exit_loc = self.end(n);
        self.push(exit, None, None, None);
        self.gap_rc = exit;
        self.mrl = self.start(n);
        self.file_exit(exit_loc);
        let mut sum = Counter::Zero;
        let mut has_default = false;
        let cases: Vec<StmtId> = ast.lists.iter(d.cases).collect();
        for &c in cases.iter().rev() {
            has_default |= st.get(c).kind() == StmtKind::DefaultStmt;
            let cc = self.rcs(c);
            sum = self.eb.add_s(sum, cc, false);
            let f = self.sub(parent, cc);
            let start = self.start(N::S(c));
            let colon = self.case_colon(c);
            let i = self.push(cc, Some(start), Some(colon), Some(f));
            self.pop(i);
        }
        sum = self.add(sum, Counter::Zero);
        if !has_default {
            let dt = self.sub(parent, sum);
            let df = self.sub(parent, dt);
            self.branch(Some(d.cond), dt, df);
        }
    }

    /// A case label's `:` (clang `getColonLoc`).
    fn case_colon(&self, c: StmtId) -> Loc {
        let mut q = 0u32;
        self.l.scan(self.ast().stmts.get(c).loc(), |k, d, _| match k {
            TokKind::Punct(Punct::Question) => {
                q += 1;
                false
            }
            TokKind::Punct(Punct::Colon) if d == 0 => {
                if q == 0 {
                    true
                } else {
                    q -= 1;
                    false
                }
            }
            _ => false,
        })
    }

    fn switch_case(&mut self, s: StmtId) {
        let st = &self.ast().stmts;
        let n = N::S(s);
        self.extend(n);
        let rc = self.rcs(s);
        let pc = self.top_count();
        let count = self.add(pc, rc);
        let start = self.start(n);
        let t = self.top();
        if t.start.is_some() && t.start == Some(start) {
            t.count = count;
        } else {
            self.push(count, Some(start), None, None);
        }
        self.gap_rc = count;
        let sub = if st.get(s).kind() == StmtKind::CaseStmt {
            let d = *st.case_data(s);
            self.visit_expr(d.lhs);
            if let Some(r) = d.rhs {
                self.visit_expr(r);
            }
            d.sub
        } else {
            st.default_sub(s)
        };
        self.visit(self.sh.norm(sub));
    }

    fn if_stmt(&mut self, s: StmtId) {
        let st = &self.ast().stmts;
        let d = *st.if_data(s);
        let n = N::S(s);
        let kw = st.get(s).loc();
        let then = d.then.map(|t| self.sh.norm(t));
        let els = d.els.map(|t| self.sh.norm(t));
        match st.if_kind(s) {
            IfKind::Consteval | IfKind::NotConsteval => {
                let Some(then) = then else { return };
                let parent = self.top_count();
                self.extend(n);
                if st.if_kind(s) == IfKind::NotConsteval {
                    self.mark_skipped(kw, self.start(then));
                    self.propagate(parent, then, true);
                    if let Some(e) = els {
                        self.mark_skipped(self.end(then), self.end(e));
                    }
                } else {
                    let to = els.map_or_else(|| self.end(then), |e| self.start(e));
                    self.mark_skipped(kw, to);
                    if let Some(e) = els {
                        self.propagate(parent, e, true);
                    }
                }
                return;
            }
            IfKind::Constexpr => {
                let Some(then) = then else { return };
                let truth = d.cond.and_then(|c| self.folds(c)).is_some_and(|v| v != 0);
                self.extend(n);
                let parent = self.top_count();
                let mut from = kw;
                if let Some(i) = d.init {
                    let i = self.sh.norm(i);
                    let (a, b) = (self.start(i), self.end(i));
                    if a.is_valid() && b.is_valid() {
                        self.mark_skipped(from, a);
                        self.propagate(parent, i, true);
                        from = b;
                    }
                }
                if truth {
                    self.mark_skipped(from, self.start(then));
                    self.propagate(parent, then, true);
                    if let Some(e) = els {
                        self.mark_skipped(self.end(then), self.end(e));
                    }
                } else {
                    let to = els.map_or_else(|| self.end(then), |e| self.start(e));
                    self.mark_skipped(from, to);
                    if let Some(e) = els {
                        self.propagate(parent, e, true);
                    }
                }
                return;
            }
            _ => {}
        }
        self.extend(n);
        self.visit_opt(d.init);
        let Some(cond) = d.cond else { return };
        let cn = N::E(cond);
        self.extend(cn);
        let parent = self.top_count();
        let tc = self.rcs(s);
        self.propagate(parent, cn, true);
        let Some(then) = then else { return };
        let g = self.gap_between(self.rparen(s), self.start(then));
        self.fill_gap(g, tc);
        self.extend(then);
        let mut out = self.propagate(tc, then, true);
        let ec = self.sub(parent, tc);
        if let Some(e) = els {
            let then_term = self.has_term;
            self.has_term = false;
            let g = self.gap_between(self.end(then), self.start(e));
            self.fill_gap(g, ec);
            self.extend(e);
            let x = self.propagate(ec, e, true);
            out = self.add(out, x);
            if then_term {
                self.has_term = true;
            }
        } else {
            out = self.add(out, ec);
        }
        if out != parent {
            self.push(out, None, None, None);
            self.gap_rc = out;
        }
        let f = self.sub(parent, tc);
        self.branch(Some(cond), tc, f);
    }

    /// `VisitAbstractConditionalOperator`.
    fn cond_op(&mut self, e: ExprId) {
        let ast = self.ast();
        let ex = &ast.exprs;
        let n = N::E(e);
        self.extend(n);
        let parent = self.top_count();
        let tc = self.rce(e);
        let (cond, t, f, mut out) = if ex.get(e).kind() == ExprKind::BinaryConditionalOperator {
            let [common, cond, t, f] = ex.binary_conditional_operands(e, &ast.lists);
            self.propagate(parent, N::E(common), true);
            (cond, t, f, tc)
        } else {
            let cond = ExprId::new(ex.get(e).a());
            let [t, f] = ex.cond_arms(e);
            self.propagate(parent, N::E(cond), true);
            let g = self.gap_between(ex.get(e).loc(), self.start(N::E(t)));
            self.fill_gap(g, tc);
            self.extend(N::E(t));
            let out = self.propagate(tc, N::E(t), true);
            (cond, t, f, out)
        };
        let _ = t;
        self.extend(N::E(f));
        let fc = self.sub(parent, tc);
        let x = self.propagate(fc, N::E(f), true);
        out = self.add(out, x);
        if out != parent {
            self.push(out, None, None, None);
            self.gap_rc = out;
        }
        let fb = self.sub(parent, tc);
        self.branch(Some(cond), tc, fb);
    }

    /// `VisitBinLAnd` / `VisitBinLOr`.
    fn logical(&mut self, e: ExprId, and: bool) {
        let ast = self.ast();
        let ex = &ast.exprs;
        let l = self.l;
        if l.sys(ex.get(e).loc()) && l.sys(ast.begin_loc(e)) && l.sys(ast.end_loc(e)) {
            self.leaf.insert(e);
            return;
        }
        let [lhs, rhs] = ex.operands(e);
        self.extend(N::E(lhs));
        let tc = self.top_count();
        let out = self.propagate(tc, N::E(lhs), true);
        let le = self.end(N::E(lhs));
        self.file_exit(le);
        self.extend(N::E(rhs));
        let rexec = self.rce(e);
        self.propagate(rexec, N::E(rhs), true);
        let rcnt = self.rce(rhs);
        if !and && self.folds(lhs).is_some_and(|v| v != 0) {
            self.gap_rc = out;
        }
        let parent = self.top_count();
        if and {
            let f = self.sub(parent, rexec);
            self.branch(Some(lhs), rexec, f);
            let f = self.sub(rexec, rcnt);
            self.branch(Some(rhs), rcnt, f);
        } else {
            let t = self.sub(parent, rexec);
            self.branch(Some(lhs), t, rexec);
            let t = self.sub(rexec, rcnt);
            self.branch(Some(rhs), t, rcnt);
        }
    }

    /// The regions and counter expressions, for [`finish`] (clang
    /// `write`).
    pub(crate) fn into_parts(self) -> Parts {
        Parts { regions: self.regions, exprs: self.eb.exprs }
    }
}

/// What [`Builder`] collected.
pub(crate) struct Parts {
    pub regions: Vec<Reg>,
    pub exprs: Vec<CExpr>,
}

/// clang `CodeGenFunction::stripCond`.
fn strip_cond(ast: &Ast, mut c: ExprId) -> ExprId {
    let ex = &ast.exprs;
    for _ in 0..WALK {
        c = ignore_parens(ast, c);
        if ex.get(c).kind() == ExprKind::UnaryOperator
            && ex.unary_op(c) == kiln_ast::expr::UnaryOp::LNot
        {
            c = ex.operand(c);
        } else {
            break;
        }
    }
    c
}

/// `EmptyCoverageMappingBuilder`: the zero mapping of the unused
/// function `d`.
pub(crate) fn empty(ast: &Ast, l: Locs<'_>, d: DeclId, files: &mut dyn Files) -> Option<Vec<u8>> {
    let body = ast.decls.function(d)?.body?;
    let sh = Shape { ast, l };
    let bn = sh.norm(body);
    let (mut s, mut e) = (sh.start(bn), sh.end(bn));
    if !l.same(s, e) {
        let mut sf = l.fid(s);
        let ef0 = l.fid(e);
        for _ in 0..WALK {
            if sf == ef0 || l.nested_in(e, sf) {
                break;
            }
            s = l.inc_or_exp(s);
            if !s.is_valid() {
                return None;
            }
            sf = l.fid(s);
        }
        for _ in 0..WALK {
            if l.fid(e) == sf {
                break;
            }
            e = l.tok_end(l.inc_or_exp(e));
            if !e.is_valid() {
                return None;
            }
        }
    }
    let mut regions = vec![Reg::new(Counter::Zero, Some(s), Some(e))];
    finish(l, &mut regions, &[], false, files)
}

/// `gatherFileIDs`, `emitExpansionRegions`, `emitSourceRegions`,
/// `gatherSkippedRegions` and the writer: the encoded mapping, or
/// `None` when it has no region.
pub(crate) fn finish(
    l: Locs<'_>,
    regions: &mut [Reg],
    exprs: &[CExpr],
    full: bool,
    files: &mut dyn Files,
) -> Option<Vec<u8>> {
    let sm = l.sm;
    // gatherFileIDs.
    let mut seen: FxHashSet<(Loc, Loc)> = FxHashSet::default();
    let mut locs: Vec<(Loc, usize)> = Vec::new();
    for r in regions.iter_mut() {
        let (Some(mut loc), Some(_)) = (r.start, r.end) else { continue };
        let (ns, end) = l.non_scratch(loc);
        if let Some(end) = end {
            loc = ns;
            r.start = Some(loc);
            r.end = Some(end);
        }
        if loc.is_macro() && l.sys(loc) {
            let b = sm.spelling_loc(loc);
            let e = sm.spelling_loc(r.end.unwrap_or(Loc::INVALID));
            if l.same(b, e) {
                loc = file_loc(l, loc);
                r.start = Some(loc);
                r.end = r.end.map(|e| file_loc(l, e));
            }
        }
        let Some(f) = l.fid(loc) else { continue };
        if !seen.insert(f) {
            continue;
        }
        let mut depth = 0;
        let mut p = l.inc_or_exp(loc);
        while p.is_valid() && depth < WALK {
            depth += 1;
            p = l.inc_or_exp(p);
        }
        locs.push((loc, depth));
    }
    locs.sort_by_key(|x| x.1);
    let mut map: FxHashMap<(Loc, Loc), (u32, Loc)> = FxHashMap::default();
    let mut vfiles: Vec<u32> = Vec::new();
    let mut entries: Vec<(FileId, Loc)> = Vec::new();
    for (loc, _) in locs {
        let Some((sf, _)) = sm.offset_of(sm.spelling_loc(loc)) else { continue };
        if sm.file_name(sf).starts_with('<') {
            continue;
        }
        let Some(f) = l.fid(loc) else { continue };
        map.insert(f, (vfiles.len() as u32, loc));
        vfiles.push(files.global_id(sf));
        if loc.is_file() {
            entries.push((sf, f.0));
        }
    }
    let cov = |x: Loc| l.fid(x).and_then(|f| map.get(&f)).map(|m| m.0);
    let mut out: Vec<MapRegion> = Vec::new();
    let mk = |kind, count, fc, file, l1: (u32, u32), l2: (u32, u32)| MapRegion {
        kind,
        count,
        false_count: fc,
        file,
        expanded: 0,
        l1: l1.0,
        c1: l1.1,
        l2: l2.0,
        c2: l2.1,
    };
    // emitExpansionRegions.
    let mut filter: FxHashSet<(Loc, Loc)> = FxHashSet::default();
    if full {
        let mut fm: Vec<(u32, Loc)> = map.values().copied().collect();
        fm.sort_by_key(|x| x.0);
        for (id, loc) in fm {
            let mut parent = l.inc_or_exp(loc);
            if parent.is_macro() {
                parent = l.non_scratch(parent).0;
            }
            if !parent.is_valid() {
                continue;
            }
            let Some(pid) = cov(parent) else { continue };
            let end = l.tok_end(parent);
            filter.insert((parent, end));
            let (Some(a), Some(b)) = (l.lc(parent), l.lc(end)) else { continue };
            let mut r = mk(Kind::Expansion, Counter::Zero, Counter::Zero, pid, a, b);
            r.expanded = id;
            out.push(r);
        }
    }
    // emitSourceRegions.
    for r in regions.iter() {
        let (Some(s), Some(e)) = (r.start, r.end) else { continue };
        if l.sys(s) {
            continue;
        }
        let Some(id) = cov(s) else { continue };
        if filter.contains(&(s, e)) {
            continue;
        }
        let (Some(a), Some(b)) = (l.lc(s), l.lc(e)) else { continue };
        let x = if r.gap {
            mk(Kind::Gap, r.count, Counter::Zero, id, a, b)
        } else if r.skipped {
            mk(Kind::Skipped, Counter::Zero, Counter::Zero, id, a, b)
        } else if let Some(f) = r.fcount {
            mk(Kind::Branch, r.count, f, id, a, b)
        } else {
            mk(Kind::Code, r.count, Counter::Zero, id, a, b)
        };
        out.push(x);
    }
    // gatherSkippedRegions.
    if full {
        let mut lines = vec![(u32::MAX, 0u32); vfiles.len()];
        for r in &out {
            let x = &mut lines[r.file as usize];
            x.0 = x.0.min(r.l1);
            x.1 = x.1.max(r.l2);
        }
        for (file, base) in entries {
            let Some(&(id, _)) = l.fid(base).and_then(|f| map.get(&f)) else { continue };
            for sk in files.skipped(file, base) {
                let (Some(a), Some(b)) = (l.lc(sk.lo), l.lc(sk.hi)) else { continue };
                let (mut a, mut b) = (a, b);
                if sk.kind == SkipKind::Comment {
                    a.1 = 1;
                    let tok_line = |t: Option<super::skipped::Tok>, at: Loc| {
                        t.and_then(|t| t.loc).filter(|&x| l.same(at, x)).and_then(|x| l.lc(x)).map(|x| x.0)
                    };
                    if tok_line(sk.prev, sk.lo) == Some(a.0) {
                        a.0 += 1;
                    }
                    if tok_line(sk.next, sk.hi) == Some(b.0) {
                        b.0 -= 1;
                        b.1 += 1;
                    }
                    if a > b {
                        continue;
                    }
                }
                let (lo, hi) = lines[id as usize];
                if a.0 >= lo && b.0 <= hi {
                    out.push(mk(Kind::Skipped, Counter::Zero, Counter::Zero, id, a, b));
                }
            }
        }
    }
    if out.is_empty() {
        return None;
    }
    let mut buf = Vec::new();
    super::encode::write_mapping(&vfiles, exprs, &mut out, &mut buf);
    Some(buf)
}

/// `SourceManager::getFileLoc`.
fn file_loc(l: Locs<'_>, mut loc: Loc) -> Loc {
    for _ in 0..WALK {
        if !loc.is_macro() {
            break;
        }
        loc = if l.is_macro_arg(loc) { l.sm.immediate_spelling_loc(loc) } else { l.imm_exp(loc).0 };
    }
    loc
}
