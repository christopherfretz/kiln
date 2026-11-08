//! The control-flow graph of a function body (clang `CFG`; spec/sema.md
//! §7.5, docs/design/reachability.md). One graph is built per analysed
//! body, after the body is complete and only when a flow-sensitive
//! consumer (or `-fkiln-dump-cfg`) asks for it, and every consumer
//! queries the same graph: the runtime-behavior filter, fall-off-the-end
//! (`-Wreturn-type`, `-Winvalid-noreturn`, `-Wmissing-noreturn`),
//! `-Wexceptions`, `-Wimplicit-fallthrough`, `-Wunreachable-code*` and
//! `-Winfinite-recursion`.
//!
//! The graph is index-based, as the AST arenas are: blocks, elements and
//! edges live in flat vectors and a block holds ranges into them. There
//! is no `Rc`, no pointer graph and no `unsafe`. The vectors are reused
//! from body to body (a per-`Sema` scratch that is cleared, not freed).
//!
//! An edge carries two reachability bits. `LIVE` is clang's
//! `PruneTriviallyFalseEdges` CFG: a condition that `tryEvaluateBool`
//! folds ([`truth`]) prunes the other edge, which stays in the graph as
//! an unreachable alternate. `SCAN` is the edge as clang's
//! unreachable-code scan follows it (`reachable_code::
//! FindUnreachableCode`): a configuration value does not prune, an
//! `if constexpr` prunes nothing, every case of a switch is reached.

mod body;
mod build;
mod dump;
mod query;
pub(crate) mod truth;

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use kiln_ast::{DeclId, ExprId, StmtId};

pub(crate) use body::BodyCfg;
pub(crate) use build::CfgReq;
pub(crate) use query::Unit;

/// The default `-fkiln-cfg-max-elems`: at about 16 bytes per element
/// and a block and two edges per four elements, a graph at the cap holds
/// under 1 GiB.
pub const DEFAULT_MAX_ELEMS: usize = 32 << 20;

/// A block of a [`Cfg`]: [`BlockId::ENTRY`], [`BlockId::EXIT`], then the
/// body's blocks in creation (program) order.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub(crate) struct BlockId(pub(crate) u32);

impl BlockId {
    pub(crate) const ENTRY: BlockId = BlockId(0);
    pub(crate) const EXIT: BlockId = BlockId(1);
    /// An edge whose target is not known yet.
    const PENDING: BlockId = BlockId(u32::MAX);

    pub(crate) fn idx(self) -> usize {
        self.0 as usize
    }
}

/// One element of a block, in evaluation order (clang `CFGElement`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Elem {
    /// A statement the graph holds as an element or terminator: a
    /// declaration of a variable, `return`, `co_return`, `break`,
    /// `continue`, `goto`, `goto *`, a `[[fallthrough]]` statement.
    Stmt(StmtId),
    /// An expression, after its operands.
    Expr(ExprId),
    /// The `n`th ctor-initializer of the constructor (clang
    /// `CFGInitializer`), after its expression.
    Init(u32),
    /// The implicit destructor call of an automatic variable at the end
    /// of its scope, or before a jump out of it (clang
    /// `CFGAutomaticObjDtor`).
    AutoDtor(DeclId),
    /// The destructor of the temporary bound by a `CXXBindTemporaryExpr`
    /// at the end of its full-expression (clang `CFGTemporaryDtor`).
    TempDtor(ExprId),
}

impl Elem {
    /// The unreachable-code scan's view of the element: statements and
    /// expressions (destructors and initializers are not scanned).
    pub(crate) fn unit(self) -> Option<Unit> {
        match self {
            Elem::Stmt(s) => Some(Unit::Stmt(s)),
            Elem::Expr(e) => Some(Unit::Expr(e)),
            Elem::Init(_) | Elem::AutoDtor(_) | Elem::TempDtor(_) => None,
        }
    }
}

/// Edge flags.
pub(crate) mod edge {
    /// Followed by the reachability of clang's pruned CFG.
    pub(crate) const LIVE: u8 = 1;
    /// Followed by the unreachable-code scan.
    pub(crate) const SCAN: u8 = 2;
    /// A pruned loop edge clang drops outright (dumped as `NULL`, no
    /// predecessor entry); other pruned edges are unreachable alternates.
    pub(crate) const NULL: u8 = 4;
    /// A `return` (or `co_return`) into the exit.
    pub(crate) const RETURN: u8 = 8;
    /// A throw-expression leaving the body (clang's fake edge).
    pub(crate) const FAKE: u8 = 16;
    /// A `noreturn` call or destructor, or a dispatch without a
    /// catch-all leaving the body (clang's abnormal edge).
    pub(crate) const ABNORMAL: u8 = 32;
    /// Both reachability bits.
    pub(crate) const BOTH: u8 = LIVE | SCAN;
}

/// An edge, as built.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Edge {
    pub(crate) from: BlockId,
    pub(crate) to: BlockId,
    pub(crate) flags: u8,
}

/// A successor or predecessor of a block.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Adj {
    pub(crate) block: BlockId,
    pub(crate) flags: u8,
}

/// The statement or expression that ends a block with a branch.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum Term {
    #[default]
    None,
    If(StmtId),
    /// The condition of `while`, `for`, `do` or a range-based `for`.
    Loop(StmtId),
    Switch(StmtId),
    /// The dispatch block of a try statement: edges to its handlers, and
    /// (without a catch-all) to the enclosing dispatch or the exit.
    Try(StmtId),
    /// `&&` or `||`: its left operand ends the block.
    Logical(ExprId),
    /// `?:`: its condition ends the block.
    Cond(ExprId),
    /// clang `TemporaryDtorsBranch`: whether the temporaries of a
    /// conditional operand (this `CXXBindTemporaryExpr` the first) were
    /// constructed, at the end of the full-expression.
    TempDtor(ExprId),
    /// `break`, `continue`, `goto`.
    Jump(StmtId),
    /// `asm goto`.
    Asm(StmtId),
    /// The shared block of every `goto *p`, with an edge to each
    /// address-taken label.
    IndirectGoto,
}

/// A basic block.
#[derive(Clone, Debug, Default)]
pub(crate) struct Block {
    pub(crate) elems: Range<u32>,
    pub(crate) succs: Range<u32>,
    pub(crate) preds: Range<u32>,
    pub(crate) term: Term,
    /// The label, case, default or handler that starts the block.
    pub(crate) label: Option<StmtId>,
    /// Ends in a `noreturn` call or destructor.
    pub(crate) noreturn: bool,
}

/// A point of the body as the builder saw it: inside a block, or between
/// blocks on a set of (possibly unresolved) edges, none when the point is
/// unreachable.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Pos {
    Block(BlockId),
    /// `pos_edges[start..start + len]`.
    Edges(u32, u32),
}

/// Why a body has no graph.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum Status {
    #[default]
    Ok,
    /// clang builds none (an `if consteval` under `-fclang-compat=19`).
    NoCfg,
    /// More elements than `-fkiln-cfg-max-elems`.
    TooLarge,
}

/// A throw-expression of the body.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Throw {
    pub(crate) expr: ExprId,
    pub(crate) block: BlockId,
    /// The innermost enclosing try statement ([`Cfg::tries`]).
    pub(crate) try_: Option<u32>,
}

/// A try statement of the body.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TryInfo {
    pub(crate) stmt: StmtId,
    pub(crate) dispatch: BlockId,
    pub(crate) outer: Option<u32>,
}

/// The graph of one function body, with the side tables its consumers
/// asked for ([`CfgReq`]).
#[derive(Default, Debug)]
pub(crate) struct Cfg {
    pub(crate) status: Status,
    pub(crate) body: Option<StmtId>,
    pub(crate) func: Option<DeclId>,
    pub(crate) blocks: Vec<Block>,
    pub(crate) elems: Vec<Elem>,
    pub(crate) edges: Vec<Edge>,
    pub(crate) succs: Vec<Adj>,
    pub(crate) preds: Vec<Adj>,
    pub(crate) pos_edges: Vec<u32>,
    pub(crate) throws: Vec<Throw>,
    pub(crate) tries: Vec<TryInfo>,
    /// The blocks a case or default label starts.
    pub(crate) case_blocks: Vec<BlockId>,
    /// Each requested expression, where the builder met it.
    pub(crate) probes: Vec<(ExprId, Pos)>,
    /// Each statement, where control enters and leaves it (`CfgReq::flow`).
    pub(crate) flow: Vec<(StmtId, Pos, Pos)>,
    pub(crate) inc: HashMap<Unit, StmtId>,
    pub(crate) ret: HashMap<Unit, StmtId>,
    pub(crate) do_conds: HashSet<ExprId>,
    /// The comma operators of the body, which have no element (with
    /// [`CfgReq::uninit`]).
    pub(crate) commas: Vec<ExprId>,
    /// Each variable of a multi-variable declaration statement with the
    /// index of the first element after its initializer, where clang's
    /// split declaration of it stands (with [`CfgReq::uninit`]).
    pub(crate) decl_marks: Vec<(u32, DeclId)>,
}

impl Cfg {
    /// Clears the graph, keeping the allocations.
    pub(crate) fn clear(&mut self) {
        self.status = Status::Ok;
        self.body = None;
        self.func = None;
        self.blocks.clear();
        self.elems.clear();
        self.edges.clear();
        self.succs.clear();
        self.preds.clear();
        self.pos_edges.clear();
        self.throws.clear();
        self.tries.clear();
        self.case_blocks.clear();
        self.probes.clear();
        self.flow.clear();
        self.inc.clear();
        self.ret.clear();
        self.do_conds.clear();
        self.commas.clear();
        self.decl_marks.clear();
    }

    /// The graph exists (clang built one, under the size cap).
    pub(crate) fn ok(&self) -> bool {
        self.status == Status::Ok
    }

    pub(crate) fn block(&self, b: BlockId) -> &Block {
        &self.blocks[b.idx()]
    }

    pub(crate) fn succs(&self, b: BlockId) -> &[Adj] {
        let r = &self.blocks[b.idx()].succs;
        &self.succs[r.start as usize..r.end as usize]
    }

    pub(crate) fn preds(&self, b: BlockId) -> &[Adj] {
        let r = &self.blocks[b.idx()].preds;
        &self.preds[r.start as usize..r.end as usize]
    }

    pub(crate) fn elems(&self, b: BlockId) -> &[Elem] {
        let r = &self.blocks[b.idx()].elems;
        &self.elems[r.start as usize..r.end as usize]
    }

    /// The blocks reachable from `seeds` over the edges carrying `mode`.
    pub(crate) fn reach(&self, mode: u8, seeds: &[BlockId]) -> Vec<bool> {
        let mut seen = vec![false; self.blocks.len()];
        let mut work: Vec<BlockId> = Vec::new();
        for &s in seeds {
            if !seen[s.idx()] {
                seen[s.idx()] = true;
                work.push(s);
            }
        }
        while let Some(b) = work.pop() {
            for a in self.succs(b) {
                if a.flags & mode != 0 && !seen[a.block.idx()] {
                    seen[a.block.idx()] = true;
                    work.push(a.block);
                }
            }
        }
        seen
    }

    /// Whether point `p` is reached, given the reached blocks `r` and the
    /// edge bit `mode`.
    pub(crate) fn pos_live(&self, p: Pos, r: &[bool], mode: u8) -> bool {
        match p {
            Pos::Block(b) => r[b.idx()],
            Pos::Edges(s, l) => self.pos_edges[s as usize..(s + l) as usize].iter().any(|&e| {
                let e = self.edges[e as usize];
                e.flags & mode != 0 && r[e.from.idx()]
            }),
        }
    }

    /// The dispatch blocks of the body's try statements.
    pub(crate) fn dispatch_blocks(&self) -> impl Iterator<Item = BlockId> + '_ {
        self.tries.iter().map(|t| t.dispatch)
    }

    /// Bytes held by the graph's vectors (the perf table's
    /// bytes-per-block figure).
    pub(crate) fn bytes(&self) -> usize {
        use std::mem::size_of;
        self.blocks.len() * size_of::<Block>()
            + self.elems.len() * size_of::<Elem>()
            + self.edges.len() * size_of::<Edge>()
            + (self.succs.len() + self.preds.len()) * size_of::<Adj>()
    }
}
