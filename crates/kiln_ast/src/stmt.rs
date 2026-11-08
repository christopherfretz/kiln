//! Statements: the 16-byte [`StmtNode`] header and side records
//! (spec/ast.md §6).
#![warn(missing_docs)]

use kiln_source::Loc;

use kiln_source::symbol::Symbol;

use crate::ids::{DeclId, ExprId, ListId, RawId, StmtId, StringLitId};
use crate::mem::{HeapSize, MemReport, VecDims};

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

/// The kind of a contract assertion ([basic.contract.general], P2900;
/// spec/contracts.md §4): `ContractAssertStmt.aux & ContractKind::MASK`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u16)]
pub enum ContractKind {
    /// An assertion-statement, `contract_assert(p);` ([stmt.contract.assert]).
    Assert = 0,
    /// A precondition specifier, `pre(p)` ([dcl.contract.func]).
    Pre = 1,
    /// A postcondition specifier, `post(r: p)` ([dcl.contract.func]).
    Post = 2,
}

impl ContractKind {
    /// The bits of `aux` holding the kind.
    pub const MASK: u16 = 3;
    /// The first `aux` bit of the evaluation-semantic slot (two bits, 0 =
    /// the translation unit's `-fcontract-evaluation-semantic=`; resolved
    /// at evaluation, WP-707).
    pub const SEMANTIC_SHIFT: u16 = 2;

    /// The kind held in `aux`.
    pub const fn from_aux(aux: u16) -> Self {
        match aux & Self::MASK {
            1 => ContractKind::Pre,
            2 => ContractKind::Post,
            _ => ContractKind::Assert,
        }
    }

    /// The keyword spelling (`contract_assert`, `pre`, `post`).
    pub const fn keyword(self) -> &'static str {
        match self {
            ContractKind::Assert => "contract_assert",
            ContractKind::Pre => "pre",
            ContractKind::Post => "post",
        }
    }

    /// The ast-dump word (spec/contracts.md §7).
    pub const fn name(self) -> &'static str {
        match self {
            ContractKind::Assert => "assert",
            ContractKind::Pre => "pre",
            ContractKind::Post => "post",
        }
    }
}

/// A contract evaluation semantic ([basic.contract.eval]/1, P2900;
/// spec/contracts.md §6): `-fcontract-evaluation-semantic=`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub enum ContractSemantic {
    /// The predicate is not evaluated.
    Ignore,
    /// A violation calls the handler, then execution continues.
    Observe,
    /// A violation calls the handler, then the program terminates (the
    /// default, as GCC's).
    #[default]
    Enforce,
    /// A violation terminates at once, with no handler.
    QuickEnforce,
}

impl ContractSemantic {
    /// The semantic the driver flag value `s` names.
    pub fn from_flag(s: &str) -> Option<Self> {
        Some(match s {
            "ignore" => ContractSemantic::Ignore,
            "observe" => ContractSemantic::Observe,
            "enforce" => ContractSemantic::Enforce,
            "quick_enforce" | "quick-enforce" => ContractSemantic::QuickEnforce,
            _ => return None,
        })
    }

    /// Whether the semantic checks the predicate at all.
    pub const fn checks(self) -> bool {
        !matches!(self, ContractSemantic::Ignore)
    }
}

/// Where a contract assertion is and what it says, for the
/// `contract_violation` object codegen builds (spec/contracts.md §6.3):
/// the `const char *` values point to frozen string literal objects.
#[derive(Clone, Copy, Debug)]
pub struct ContractSite {
    /// The presumed file name of the assertion's keyword.
    pub file: crate::ConstValueId,
    /// `__PRETTY_FUNCTION__` of the function the assertion belongs to.
    pub function: crate::ConstValueId,
    /// The predicate's source text.
    pub comment: crate::ConstValueId,
    /// The presumed line of the keyword.
    pub line: u32,
    /// The column of the keyword.
    pub column: u32,
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

/// Side record of a `CXXForRangeStmt`: clang's desugaring of
/// [stmt.ranged]/1, every slot kept. `begin`, `end`, `cond` and `inc`
/// are `None` while the range is type-dependent (a template pattern).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RangeForData {
    /// The C++20 init-statement.
    pub init: Option<StmtId>,
    /// `auto&& __range = for-range-initializer;`
    pub range: DeclId,
    /// `auto __begin = begin-expr;`
    pub begin: Option<DeclId>,
    /// `auto __end = end-expr;`
    pub end: Option<DeclId>,
    /// `__begin != __end`
    pub cond: Option<ExprId>,
    /// `++__begin`
    pub inc: Option<ExprId>,
    /// The for-range-declaration, initialized from `*__begin`.
    pub loop_var: DeclId,
    /// The body.
    pub body: StmtId,
    /// The `:` (where the implicit `__begin`/`__end` variables and
    /// expressions are located; diagnostics at instantiation).
    pub colon: Loc,
}

/// The form of an expansion statement ([stmt.expand]/3, P1306R5):
/// `CXXExpansionStmt.aux & ExpansionForm::MASK`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
#[repr(u16)]
pub enum ExpansionForm {
    /// Not known yet: the expansion-initializer is type-dependent.
    #[default]
    Dependent = 0,
    /// Over an expansion-init-list `{ a, b }`.
    Enumerating = 1,
    /// Over an expansion-iterable expression (`begin`/`end`).
    Iterating = 2,
    /// Over the elements of a structured binding declaration.
    Destructuring = 3,
}

impl ExpansionForm {
    /// The bits of `aux` holding the form.
    pub const MASK: u16 = 3;
    /// `aux` bit: the statement was expanded (its `prelude` and
    /// `expansions` are set; its pattern parts are not to be walked).
    pub const EXPANDED: u16 = 4;

    /// The form held in `aux`.
    pub const fn from_aux(aux: u16) -> Self {
        match aux & Self::MASK {
            1 => ExpansionForm::Enumerating,
            2 => ExpansionForm::Iterating,
            3 => ExpansionForm::Destructuring,
            _ => ExpansionForm::Dependent,
        }
    }

    /// The name the dump uses.
    pub const fn name(self) -> &'static str {
        match self {
            ExpansionForm::Dependent => "dependent",
            ExpansionForm::Enumerating => "enumerating",
            ExpansionForm::Iterating => "iterating",
            ExpansionForm::Destructuring => "destructuring",
        }
    }
}

/// Side record of a `CXXExpansionStmt` ([stmt.expand], P1306R5). The
/// pattern parts (`decl`, `body`, and the initializer) are a template
/// pattern at the depth in the node's `b`; once expanded, `prelude` and
/// `expansions` are the statement's code ([stmt.expand]/5) and the
/// pattern parts are kept only for the dump.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ExpansionData {
    /// The init-statement.
    pub init: Option<StmtId>,
    /// The for-range-declaration, initialized from a
    /// `CXXExpansionSelectExpr` (the pattern of each element's variable).
    pub decl: DeclId,
    /// The expressions of an expansion-init-list (a list of `ExprId`;
    /// enumerating form).
    pub exprs: ListId,
    /// The expansion-initializer expression (`None` for an
    /// expansion-init-list).
    pub range_init: Option<ExprId>,
    /// The compound-statement (a pattern).
    pub body: StmtId,
    /// Expanded: the declaration statements before the first element
    /// (`range` and `begin`, or the structured binding declaration; a
    /// list of `StmtId`).
    pub prelude: ListId,
    /// Expanded: one `CompoundStmt` per element, `{ decl = element;
    /// body }` (a list of `StmtId`).
    pub expansions: ListId,
    /// The `:`.
    pub colon: Loc,
}

/// Side record of a `CoroutineBodyStmt` ([dcl.fct.def.coroutine]/5;
/// clang's `CoroutineBodyStmt` slots): the function body as written and
/// the statements and expressions Sema builds around it. Built only for
/// a coroutine that is not a template pattern.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CoroutineBodyData {
    /// The function body as written.
    pub body: StmtId,
    /// The promise object `__promise`.
    pub promise: DeclId,
    /// The `DeclStmt` declaring [`Self::promise`].
    pub promise_stmt: StmtId,
    /// `co_await __promise.initial_suspend()` (an implicit `CoawaitExpr`,
    /// a full-expression).
    pub init_suspend: ExprId,
    /// `co_await __promise.final_suspend()`.
    pub final_suspend: ExprId,
    /// `__promise.unhandled_exception();` (an `ExprStmt`), run by the
    /// implicit `catch (...)` around the body; `None` without exceptions.
    pub on_exception: Option<StmtId>,
    /// The implicit `co_return;` when the promise has `return_void`.
    pub on_fallthrough: Option<StmtId>,
    /// The allocation call `operator new(__builtin_coro_size(), ...)`.
    pub allocate: ExprId,
    /// The deallocation call `operator delete(__builtin_coro_free(
    /// __builtin_coro_frame()), ...)`.
    pub deallocate: ExprId,
    /// `__promise.get_return_object()`: the initializer of the returned
    /// object (converted to the return type when it is initialized
    /// directly, [`Self::result_decl`] `None`).
    pub return_value: ExprId,
    /// The `__coro_gro` variable holding the result of
    /// `get_return_object()` when its type is not the return type.
    pub result_decl: Option<DeclId>,
    /// The return to the caller at the first suspension: `return
    /// __coro_gro;` (converted to the return type) with a
    /// [`Self::result_decl`], `return get_return_object();` when its type
    /// is the return type, `None` when the function returns `void`.
    pub return_stmt: Option<StmtId>,
    /// `return P::get_return_object_on_allocation_failure();`.
    pub return_on_alloc_failure: Option<StmtId>,
    /// The parameter copies ([dcl.fct.def.coroutine]/13): a `ListId` of
    /// `DeclStmt`s, one per parameter, in parameter order.
    pub param_moves: ListId,
}

/// One operand of a `GCCAsmStmt` ([dcl.asm], GNU extended asm): an
/// output or input `[name] "constraint" (expr)`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AsmOperand {
    /// The symbolic name (`[name]`), if any.
    pub name: Option<Symbol>,
    /// The constraint string (an ordinary literal).
    pub constraint: StringLitId,
    /// The constraint literal's location.
    pub constraint_loc: Loc,
    /// The operand expression (an lvalue for outputs).
    pub expr: ExprId,
}

/// Side record of a `GCCAsmStmt` (GNU extended asm; `aux` bit 0 =
/// `volatile`, bit 1 = `goto`, bit 2 = `inline`, bit 3 = basic asm
/// without any `:`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AsmData {
    /// The assembler template (an ordinary literal).
    pub string: StringLitId,
    /// The template literal's first token location.
    pub string_loc: Loc,
    /// Index of the first operand in the operand table (outputs first,
    /// then inputs).
    pub operands: u32,
    /// The number of outputs.
    pub num_outputs: u32,
    /// The number of inputs.
    pub num_inputs: u32,
    /// The clobber strings (a `ListId` of `StringLitId`s).
    pub clobbers: ListId,
    /// The `asm goto` labels (a `ListId` of `Label` `DeclId`s).
    pub labels: ListId,
    /// The closing `)`.
    pub rparen: Loc,
    /// clang's `!srcloc` values (a raw `u32` list): the template's begin
    /// location then each later line's first byte, as clang's raw
    /// `SourceLocation` encoding of a main-file location (offset + 2).
    pub srclocs: ListId,
}

/// `GCCAsmStmt` `aux` bits.
pub mod asm_bits {
    /// `asm volatile`.
    pub const VOLATILE: u16 = 1;
    /// `asm goto`.
    pub const GOTO: u16 = 2;
    /// `asm inline`.
    pub const INLINE: u16 = 4;
    /// Basic asm (no `:` at all).
    pub const BASIC: u16 = 8;
}

/// Statement headers and side records. Index 0 of every vector is a dummy.
#[derive(Debug)]
pub struct StmtArena {
    nodes: Vec<StmtNode>,
    ifs: Vec<IfData>,
    switches: Vec<SwitchData>,
    cases: Vec<CaseData>,
    loops: Vec<LoopData>,
    range_fors: Vec<RangeForData>,
    expansions: Vec<ExpansionData>,
    asms: Vec<AsmData>,
    asm_operands: Vec<AsmOperand>,
    coros: Vec<CoroutineBodyData>,
    /// The FP pragma overrides compound statements record (index 0: none;
    /// WP-806, spec/codegen.md "Floating-point semantics").
    fp_overrides: Vec<crate::fp::FpOverride>,
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
            range_fors: vec![RangeForData {
                init: None,
                range: DeclId::new(DeclId::MAX_INDEX),
                begin: None,
                end: None,
                cond: None,
                inc: None,
                loop_var: DeclId::new(DeclId::MAX_INDEX),
                body: ds,
                colon: Loc::INVALID,
            }],
            expansions: vec![ExpansionData {
                init: None,
                decl: DeclId::new(DeclId::MAX_INDEX),
                exprs: ListId::EMPTY,
                range_init: None,
                body: ds,
                prelude: ListId::EMPTY,
                expansions: ListId::EMPTY,
                colon: Loc::INVALID,
            }],
            asms: vec![AsmData {
                string: StringLitId::new(StringLitId::MAX_INDEX),
                string_loc: Loc::INVALID,
                operands: 0,
                num_outputs: 0,
                num_inputs: 0,
                clobbers: ListId::EMPTY,
                labels: ListId::EMPTY,
                rparen: Loc::INVALID,
                srclocs: ListId::EMPTY,
            }],
            asm_operands: Vec::new(),
            coros: vec![CoroutineBodyData {
                body: ds,
                promise: DeclId::new(DeclId::MAX_INDEX),
                promise_stmt: ds,
                init_suspend: de,
                final_suspend: de,
                on_exception: None,
                on_fallthrough: None,
                allocate: de,
                deallocate: de,
                return_value: de,
                result_decl: None,
                return_stmt: None,
                return_on_alloc_failure: None,
                param_moves: ListId::EMPTY,
            }],
            fp_overrides: vec![crate::fp::FpOverride::NONE],
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
    pub(crate) fn clear_return_nrvo(&mut self, s: StmtId) {
        assert_eq!(self.get(s).kind, StmtKind::ReturnStmt, "not a ReturnStmt");
        self.nodes[s.index() as usize].b = 0;
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
    pub(crate) fn push_range_for(&mut self, x: RangeForData) -> u32 {
        push_side(&mut self.range_fors, x)
    }
    pub(crate) fn push_expansion(&mut self, x: ExpansionData) -> u32 {
        push_side(&mut self.expansions, x)
    }
    pub(crate) fn push_coro(&mut self, x: CoroutineBodyData) -> u32 {
        push_side(&mut self.coros, x)
    }
    /// The index of FP override `o` in the table (0 for none; equal
    /// overrides share an index).
    pub(crate) fn intern_fp(&mut self, o: crate::fp::FpOverride) -> u16 {
        if o.is_empty() {
            return 0;
        }
        if let Some(i) = self.fp_overrides.iter().position(|x| *x == o) {
            return i as u16;
        }
        // 7 mask bits x 3 contraction modes bound the distinct overrides.
        assert!(self.fp_overrides.len() < u16::MAX as usize, "FP override space exhausted");
        self.fp_overrides.push(o);
        (self.fp_overrides.len() - 1) as u16
    }
    pub(crate) fn push_asm(&mut self, mut x: AsmData, ops: &[AsmOperand]) -> u32 {
        x.operands = self.asm_operands.len() as u32;
        assert!(
            self.asm_operands.len() + ops.len() < u32::MAX as usize,
            "asm operand space exhausted"
        );
        self.asm_operands.extend_from_slice(ops);
        push_side(&mut self.asms, x)
    }

    fn expect(&self, s: StmtId, kinds: &[StmtKind]) -> &StmtNode {
        // Invariant (WP-145): accessors are called on the kind they read;
        // a mismatch is a Sema/AST bug, never input (spec/ast.md).
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

    /// The side record of a `CXXForRangeStmt`.
    pub fn range_for_data(&self, s: StmtId) -> &RangeForData {
        &self.range_fors[self.expect(s, &[StmtKind::CXXForRangeStmt]).a as usize]
    }

    /// The side record of a `CXXExpansionStmt`.
    pub fn expansion_data(&self, s: StmtId) -> &ExpansionData {
        &self.expansions[self.expect(s, &[StmtKind::CXXExpansionStmt]).a as usize]
    }

    /// The form of a `CXXExpansionStmt`.
    pub fn expansion_form(&self, s: StmtId) -> ExpansionForm {
        ExpansionForm::from_aux(self.expect(s, &[StmtKind::CXXExpansionStmt]).aux)
    }

    /// Whether a `CXXExpansionStmt` was expanded (not a pattern).
    pub fn expansion_expanded(&self, s: StmtId) -> bool {
        self.expect(s, &[StmtKind::CXXExpansionStmt]).aux & ExpansionForm::EXPANDED != 0
    }

    /// The depth of a `CXXExpansionStmt`'s synthetic template level.
    pub fn expansion_depth(&self, s: StmtId) -> u16 {
        self.expect(s, &[StmtKind::CXXExpansionStmt]).b as u16
    }

    /// The side record of a `CoroutineBodyStmt`.
    pub fn coroutine_body(&self, s: StmtId) -> &CoroutineBodyData {
        &self.coros[self.expect(s, &[StmtKind::CoroutineBodyStmt]).a as usize]
    }

    /// The operand of a `CoreturnStmt` as written (`None` for
    /// `co_return;`).
    pub fn coreturn_operand(&self, s: StmtId) -> Option<ExprId> {
        ExprId::from_raw(self.expect(s, &[StmtKind::CoreturnStmt]).a)
    }

    /// The promise call of a `CoreturnStmt` (`__promise.return_value(e)`
    /// or `__promise.return_void()`); `None` in a template pattern or
    /// after an error.
    pub fn coreturn_promise_call(&self, s: StmtId) -> Option<ExprId> {
        ExprId::from_raw(self.expect(s, &[StmtKind::CoreturnStmt]).b)
    }

    /// Whether a `CoreturnStmt` is the implicit `co_return;` at the end
    /// of a coroutine body.
    pub fn coreturn_is_implicit(&self, s: StmtId) -> bool {
        self.expect(s, &[StmtKind::CoreturnStmt]).aux & 1 != 0
    }

    /// The kind of a `ContractAssertStmt` (P2900).
    pub fn contract_kind(&self, s: StmtId) -> ContractKind {
        ContractKind::from_aux(self.expect(s, &[StmtKind::ContractAssertStmt]).aux)
    }

    /// The predicate of a `ContractAssertStmt`, contextually converted to
    /// `bool` (a full-expression).
    pub fn contract_cond(&self, s: StmtId) -> ExprId {
        ExprId::new(self.expect(s, &[StmtKind::ContractAssertStmt]).a)
    }

    /// The `ContractResultName` of a postcondition that introduces one.
    pub fn contract_result_name(&self, s: StmtId) -> Option<DeclId> {
        DeclId::from_raw(self.expect(s, &[StmtKind::ContractAssertStmt]).b)
    }

    /// The evaluation-semantic slot of a `ContractAssertStmt` (0: the
    /// translation unit's default; spec/contracts.md §6).
    pub fn contract_semantic_slot(&self, s: StmtId) -> u16 {
        (self.expect(s, &[StmtKind::ContractAssertStmt]).aux >> ContractKind::SEMANTIC_SHIFT) & 3
    }

    /// The side record of a `GCCAsmStmt`.
    pub fn asm_data(&self, s: StmtId) -> &AsmData {
        &self.asms[self.expect(s, &[StmtKind::GCCAsmStmt]).a as usize]
    }

    /// The `aux` bits of a `GCCAsmStmt` (see [`asm_bits`]).
    pub fn asm_bits(&self, s: StmtId) -> u16 {
        self.expect(s, &[StmtKind::GCCAsmStmt]).aux
    }

    /// The operands of a `GCCAsmStmt`: outputs, then inputs.
    pub fn asm_operands(&self, s: StmtId) -> &[AsmOperand] {
        let d = self.asm_data(s);
        let lo = d.operands as usize;
        &self.asm_operands[lo..lo + (d.num_outputs + d.num_inputs) as usize]
    }

    /// The statements of a `CompoundStmt` (a `ListId` of `StmtId`s).
    pub fn compound_body(&self, s: StmtId) -> ListId {
        ListId::new(self.expect(s, &[StmtKind::CompoundStmt]).a)
    }

    /// The FP pragma override in effect in a `CompoundStmt`, relative to
    /// the command line (WP-806; [`crate::fp::FpOverride::NONE`] when no
    /// pragma applies).
    pub fn compound_fp(&self, s: StmtId) -> crate::fp::FpOverride {
        self.fp_overrides[self.expect(s, &[StmtKind::CompoundStmt]).aux as usize]
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

    /// The `Label` declaration of a `LabelStmt` or `GotoStmt`.
    pub fn label_decl(&self, s: StmtId) -> DeclId {
        DeclId::new(self.expect(s, &[StmtKind::LabelStmt, StmtKind::GotoStmt]).a)
    }

    /// The target of a GNU `IndirectGotoStmt` (`goto *e;`), converted to
    /// `const void*`.
    pub fn indirect_goto_target(&self, s: StmtId) -> ExprId {
        ExprId::new(self.expect(s, &[StmtKind::IndirectGotoStmt]).a)
    }

    /// The location of the `*` of an `IndirectGotoStmt`.
    pub fn indirect_goto_star(&self, s: StmtId) -> Loc {
        Loc::from_raw(self.expect(s, &[StmtKind::IndirectGotoStmt]).b)
    }

    /// The labelled statement of a `LabelStmt`, or the statement an
    /// `AttributedStmt`'s attributes appertain to.
    pub fn sub_stmt(&self, s: StmtId) -> StmtId {
        StmtId::new(self.expect(s, &[StmtKind::LabelStmt, StmtKind::AttributedStmt]).b)
    }

    /// The attributes (`AttrId`s) of an `AttributedStmt`.
    pub fn stmt_attrs(&self, s: StmtId) -> ListId {
        ListId::new(self.expect(s, &[StmtKind::AttributedStmt]).a)
    }

    /// The operand of a `ReturnStmt`.
    pub fn return_value(&self, s: StmtId) -> Option<ExprId> {
        ExprId::from_raw(self.expect(s, &[StmtKind::ReturnStmt]).a)
    }

    /// The NRVO candidate of a `ReturnStmt`.
    pub fn return_nrvo(&self, s: StmtId) -> Option<DeclId> {
        DeclId::from_raw(self.expect(s, &[StmtKind::ReturnStmt]).b)
    }

    /// The try-block of a `CXXTryStmt` ([except.pre]/1).
    pub fn try_block(&self, s: StmtId) -> StmtId {
        StmtId::new(self.expect(s, &[StmtKind::CXXTryStmt]).a)
    }

    /// The handlers of a `CXXTryStmt` (a `ListId` of `CXXCatchStmt`s).
    pub fn try_handlers(&self, s: StmtId) -> ListId {
        ListId::new(self.expect(s, &[StmtKind::CXXTryStmt]).b)
    }

    /// The exception variable of a `CXXCatchStmt`; `None` for `catch (...)`.
    pub fn catch_var(&self, s: StmtId) -> Option<DeclId> {
        DeclId::from_raw(self.expect(s, &[StmtKind::CXXCatchStmt]).a)
    }

    /// The compound statement of a `CXXCatchStmt`.
    pub fn catch_block(&self, s: StmtId) -> StmtId {
        StmtId::new(self.expect(s, &[StmtKind::CXXCatchStmt]).b)
    }

    pub(crate) fn live_bytes(&self) -> u64 {
        self.rows().iter().map(|r| r.live_bytes()).sum()
    }

    fn rows(&self) -> [VecDims; 10] {
        [
            VecDims::of("nodes", &self.nodes),
            VecDims::of("ifs", &self.ifs),
            VecDims::of("switches", &self.switches),
            VecDims::of("cases", &self.cases),
            VecDims::of("loops", &self.loops),
            VecDims::of("range_fors", &self.range_fors),
            VecDims::of("expansions", &self.expansions),
            VecDims::of("asms", &self.asms),
            VecDims::of("asm_operands", &self.asm_operands),
            VecDims::of("coros", &self.coros),
        ]
    }
}

impl HeapSize for StmtArena {
    fn heap_bytes(&self) -> MemReport {
        let mut r = MemReport::new();
        for row in self.rows() {
            r.push(row.row());
        }
        r
    }
}
