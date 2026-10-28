//! Statements: the 16-byte [`StmtNode`] header and side records
//! (spec/ast.md §6).
#![warn(missing_docs)]

use kiln_source::Loc;

use crate::ids::{DeclId, ExprId, ListId, RawId, StmtId};
use crate::mem::{HeapSize, MemReport, MemRow};

/// Statement kinds (spec/ast.md §6; clang's class names). The numbering is
/// stable.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
#[allow(missing_docs, reason = "named after clang's Stmt classes; see spec/ast.md §6")]
pub enum StmtKind {
    CompoundStmt,
    DeclStmt,
    ExprStmt,
    NullStmt,
    IfStmt,
    SwitchStmt,
    CaseStmt,
    DefaultStmt,
    BreakStmt,
    ContinueStmt,
    WhileStmt,
    DoStmt,
    ForStmt,
    ReturnStmt,
    LabelStmt,
    GotoStmt,
    IndirectGotoStmt,
    AttributedStmt,
    CXXForRangeStmt,
    CXXExpansionStmt,
    CXXTryStmt,
    CXXCatchStmt,
    CoroutineBodyStmt,
    CoreturnStmt,
    GCCAsmStmt,
    ContractAssertStmt,
}

/// `IfStmt.aux`: the form of the `if` ([stmt.if]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
#[repr(u16)]
pub enum IfKind {
    /// `if (c)`.
    #[default]
    Ordinary = 0,
    /// `if constexpr (c)`.
    Constexpr = 1,
    /// `if consteval`.
    Consteval = 2,
    /// `if !consteval`.
    NotConsteval = 3,
}

/// The 16-byte statement header (spec/ast.md §6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StmtNode {
    kind: StmtKind,
    flags: u8,
    aux: u16,
    loc: Loc,
    a: u32,
    b: u32,
}

impl StmtNode {
    pub(crate) const fn new(kind: StmtKind, aux: u16, loc: Loc, a: u32, b: u32) -> Self {
        StmtNode { kind, flags: 0, aux, loc, a, b }
    }
    /// The kind.
    pub const fn kind(&self) -> StmtKind {
        self.kind
    }
    /// Per-kind flags.
    pub const fn flags(&self) -> u8 {
        self.flags
    }
    /// Per-kind auxiliary value (`IfKind` for `IfStmt`).
    pub const fn aux(&self) -> u16 {
        self.aux
    }
    /// The keyword / start location.
    pub const fn loc(&self) -> Loc {
        self.loc
    }
    /// Operand word `a`.
    pub const fn a(&self) -> u32 {
        self.a
    }
    /// Operand word `b`.
    pub const fn b(&self) -> u32 {
        self.b
    }
}

/// Side record of an `IfStmt` (the dump's `has_init`, `has_var`,
/// `has_else` come from the optional slots).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct IfData {
    /// The init-statement.
    pub init: Option<StmtId>,
    /// The condition variable (`if (T x = e)`).
    pub cond_var: Option<DeclId>,
    /// The condition (`None` for `if consteval`).
    pub cond: Option<ExprId>,
    /// The then-branch (`None` when discarded in an instantiation).
    pub then: Option<StmtId>,
    /// The else-branch.
    pub els: Option<StmtId>,
}

/// Side record of a `SwitchStmt`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SwitchData {
    /// The init-statement.
    pub init: Option<StmtId>,
    /// The condition variable.
    pub cond_var: Option<DeclId>,
    /// The condition.
    pub cond: ExprId,
    /// The body.
    pub body: StmtId,
    /// The `CaseStmt`/`DefaultStmt`s of this switch, in source order.
    pub cases: ListId,
}

/// Side record of a `CaseStmt`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CaseData {
    /// The label value (a `ConstantExpr` unless value-dependent).
    pub lhs: ExprId,
    /// GNU range end.
    pub rhs: Option<ExprId>,
    /// The labelled statement.
    pub sub: StmtId,
}

/// Side record of `WhileStmt`, `DoStmt` and `ForStmt`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct LoopData {
    /// `for` init-statement.
    pub init: Option<StmtId>,
    /// Condition variable (`while (T x = e)`, `for (; T x = e;)`).
    pub cond_var: Option<DeclId>,
    /// The condition (`None` for `for (;;)`).
    pub cond: Option<ExprId>,
    /// `for` increment.
    pub inc: Option<ExprId>,
    /// The body.
    pub body: Option<StmtId>,
}

/// Statement headers and side records. Index 0 of every vector is a dummy.
#[derive(Debug)]
pub struct StmtArena {
    nodes: Vec<StmtNode>,
    ifs: Vec<IfData>,
    switches: Vec<SwitchData>,
    cases: Vec<CaseData>,
    loops: Vec<LoopData>,
}

impl Default for StmtArena {
    fn default() -> Self {
        let de = ExprId::new(ExprId::MAX_INDEX);
        let ds = StmtId::new(StmtId::MAX_INDEX);
        Self {
            nodes: vec![StmtNode::new(StmtKind::NullStmt, 0, Loc::INVALID, 0, 0)],
            ifs: vec![IfData::default()],
            switches: vec![SwitchData {
                init: None,
                cond_var: None,
                cond: de,
                body: ds,
                cases: ListId::EMPTY,
            }],
            cases: vec![CaseData { lhs: de, rhs: None, sub: ds }],
            loops: vec![LoopData::default()],
        }
    }
}

fn push_side<T>(v: &mut Vec<T>, x: T) -> u32 {
    let i = v.len();
    assert!(i < u32::MAX as usize, "statement side-record space exhausted");
    v.push(x);
    i as u32
}

impl StmtArena {
    /// An arena holding only the dummies.
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of statements.
    pub fn len(&self) -> usize {
        self.nodes.len() - 1
    }

    /// True if there are no statements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The header of `s`.
    pub fn get(&self, s: StmtId) -> &StmtNode {
        &self.nodes[s.index() as usize]
    }

    pub(crate) fn push(&mut self, n: StmtNode) -> StmtId {
        let id = StmtId::new(self.nodes.len() as u32);
        self.nodes.push(n);
        id
    }
    pub(crate) fn push_if(&mut self, x: IfData) -> u32 {
        push_side(&mut self.ifs, x)
    }
    pub(crate) fn push_switch(&mut self, x: SwitchData) -> u32 {
        push_side(&mut self.switches, x)
    }
    pub(crate) fn push_case(&mut self, x: CaseData) -> u32 {
        push_side(&mut self.cases, x)
    }
    pub(crate) fn push_loop(&mut self, x: LoopData) -> u32 {
        push_side(&mut self.loops, x)
    }

    fn expect(&self, s: StmtId, kinds: &[StmtKind]) -> &StmtNode {
        let n = self.get(s);
        assert!(kinds.contains(&n.kind), "expected one of {kinds:?}, found {:?}", n.kind);
        n
    }

    /// The side record of an `IfStmt`.
    pub fn if_data(&self, s: StmtId) -> &IfData {
        &self.ifs[self.expect(s, &[StmtKind::IfStmt]).a as usize]
    }

    /// The form of an `IfStmt`.
    pub fn if_kind(&self, s: StmtId) -> IfKind {
        match self.expect(s, &[StmtKind::IfStmt]).aux {
            1 => IfKind::Constexpr,
            2 => IfKind::Consteval,
            3 => IfKind::NotConsteval,
            _ => IfKind::Ordinary,
        }
    }

    /// The side record of a `SwitchStmt`.
    pub fn switch_data(&self, s: StmtId) -> &SwitchData {
        &self.switches[self.expect(s, &[StmtKind::SwitchStmt]).a as usize]
    }

    /// The side record of a `CaseStmt`.
    pub fn case_data(&self, s: StmtId) -> &CaseData {
        &self.cases[self.expect(s, &[StmtKind::CaseStmt]).a as usize]
    }

    /// The side record of a `WhileStmt`, `DoStmt` or `ForStmt`.
    pub fn loop_data(&self, s: StmtId) -> &LoopData {
        let n = self.expect(s, &[StmtKind::WhileStmt, StmtKind::DoStmt, StmtKind::ForStmt]);
        &self.loops[n.a as usize]
    }

    /// The statements of a `CompoundStmt` (a `ListId` of `StmtId`s).
    pub fn compound_body(&self, s: StmtId) -> ListId {
        ListId::new(self.expect(s, &[StmtKind::CompoundStmt]).a)
    }

    /// The `}` of a `CompoundStmt`.
    pub fn rbrace(&self, s: StmtId) -> Loc {
        Loc::from_raw(self.expect(s, &[StmtKind::CompoundStmt]).b)
    }

    /// The declarations of a `DeclStmt` (a `ListId` of `DeclId`s).
    pub fn decl_stmt_decls(&self, s: StmtId) -> ListId {
        ListId::new(self.expect(s, &[StmtKind::DeclStmt]).a)
    }

    /// The expression of an `ExprStmt`.
    pub fn expr_stmt_expr(&self, s: StmtId) -> ExprId {
        ExprId::new(self.expect(s, &[StmtKind::ExprStmt]).a)
    }

    /// The labelled statement of a `DefaultStmt`.
    pub fn default_sub(&self, s: StmtId) -> StmtId {
        StmtId::new(self.expect(s, &[StmtKind::DefaultStmt]).a)
    }

    /// The operand of a `ReturnStmt`.
    pub fn return_value(&self, s: StmtId) -> Option<ExprId> {
        ExprId::from_raw(self.expect(s, &[StmtKind::ReturnStmt]).a)
    }

    /// The NRVO candidate of a `ReturnStmt`.
    pub fn return_nrvo(&self, s: StmtId) -> Option<DeclId> {
        DeclId::from_raw(self.expect(s, &[StmtKind::ReturnStmt]).b)
    }

    pub(crate) fn live_bytes(&self) -> u64 {
        self.rows().iter().map(|r| r.live_bytes).sum()
    }

    fn rows(&self) -> [MemRow; 5] {
        [
            MemRow::of_vec("nodes", &self.nodes),
            MemRow::of_vec("ifs", &self.ifs),
            MemRow::of_vec("switches", &self.switches),
            MemRow::of_vec("cases", &self.cases),
            MemRow::of_vec("loops", &self.loops),
        ]
    }
}

impl HeapSize for StmtArena {
    fn heap_bytes(&self) -> MemReport {
        let mut r = MemReport::new();
        for row in self.rows() {
            r.push(row);
        }
        r
    }
}
